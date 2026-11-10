// Game-side glue for PneumaticCraft slice 1.
//
// Injected into `crate::game` as `pneumaticcraft_state`; it needs the game, so
// it is not part of the standalone library (only `runtime.rs` is).
//
// The two machines are upstream's `FluxCompressorBlockEntity` and
// `PneumaticDynamoBlockEntity` (read from the 8.2.24 jar; see
// `../../FINDINGS.md`). Each block entity keeps the API 142 FE buffer's amount
// in the saved int `energy`, its air handler's amount in `air`, and its heat
// exchanger in `temperature` (kelvin). A tick is the air handler's (disperse to
// a lower-pressure connected machine, then vent if there is none), the machine
// itself (`runtime.rs`), the dynamo's FE push into the block its coil faces, and
// the `active` block state.
//
// The air handler's saved shape is flattened (`air`, `temperature`) instead of
// upstream's nested `AirHandler` compound and per-exchanger tag; that is a
// recorded deviation, and the numbers are the same.

use horizon_client::containers::{BlockEntity, ModBlockEntity, ModEnergyBuffer};
use horizon_client::text::Text;
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, JavaString, Tag};

use crate::game::Game;
use crate::game::native_server::{NativeCommand, NativeServerPlayer};
use crate::game::pneumaticcraft::{
    AMBIENT_TEMPERATURE, ENERGY_CAPACITY, FluxCompressor, Heat,
    PneumaticDynamo, VOLUME_FLUX_COMPRESSOR, VOLUME_PNEUMATIC_DYNAMO, disperse,
    heat_efficiency, leak, pressure,
};

/// The flux compressor block and its item (`ModBlocks.FLUX_COMPRESSOR`).
pub const FLUX: &str = "pneumaticcraft:flux_compressor";
/// The pneumatic dynamo block and its item (`ModBlocks.PNEUMATIC_DYNAMO`).
pub const DYNAMO: &str = "pneumaticcraft:pneumatic_dynamo";

/// The air handler's saved amount (`AirHandler.Air` upstream).
const KEY_AIR: &str = "air";
/// The API 142 buffer's amount: the pin fixes this key (`docs/native-mods.md`).
const KEY_ENERGY: &str = "energy";
/// The heat exchanger's saved temperature, kelvin.
const KEY_TEMPERATURE: &str = "temperature";
/// The flux compressor's last 5-tick air rate (upstream's transient
/// `airPerTick` field, kept so the rate survives between ticks).
const KEY_AIR_RATE: &str = "air_rate";
/// The flux compressor's last 5-tick FE rate (`rfPerTick`).
const KEY_FE_RATE: &str = "fe_rate";
/// The whole-millibucket carry (upstream's transient `airBuffer`).
const KEY_AIR_BUFFER: &str = "air_buffer";
/// Upstream's block state (`PNCBlockStateProperties.ACTIVE`).
const PROP_ACTIVE: &str = "active";
/// Upstream's `directionProperty()`.
const PROP_FACING: &str = "facing";

/// Which slice-1 machine a block entity is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flux,
    Dynamo,
}

impl Kind {
    fn of(id: &str) -> Option<Self> {
        match id {
            FLUX => Some(Self::Flux),
            DYNAMO => Some(Self::Dynamo),
            _ => None,
        }
    }

    fn volume(self) -> i32 {
        match self {
            Self::Flux => VOLUME_FLUX_COMPRESSOR,
            Self::Dynamo => VOLUME_PNEUMATIC_DYNAMO,
        }
    }

    /// `BlockBehaviour`'s default state: no facing property is set before a
    /// placement, and the harness places with `/setblock` and an explicit one.
    fn default_facing(self) -> Direction {
        match self {
            Self::Flux => Direction::North,
            Self::Dynamo => Direction::Up,
        }
    }

    /// Upstream's `ACTIVE` refresh cadence: `& 0x7` and `& 0xf`.
    fn active_mask(self) -> u64 {
        match self {
            Self::Flux => 0x7,
            Self::Dynamo => 0xf,
        }
    }
}

// ---------------------------------------------------------------------------
// Declaration hooks
// ---------------------------------------------------------------------------

/// Both blocks have a block entity (`native_block_entity`, API 15/129).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if Kind::of(block).is_some() {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// ... and tick on the server (`native_block_entity_ticks`, API 15).
pub fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if Kind::of(block).is_some() {
        *ticks = true;
    }
}

/// The FE buffer (`native_block_entity_energy`, API 142). Upstream reaches the
/// buffer only through `getEnergyHandler(dir)`: the flux compressor on every
/// face but its `facing`, the dynamo on `facing` alone. API 142 declares faces
/// for a block id, not for a block state, so the slice takes all six and
/// records the deviation in `notes.md`; the machines' own intake is still the
/// one their ticks model.
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_block_entity_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
    if Kind::of(block).is_none() {
        return;
    }
    *energy = Some(ModEnergyBuffer {
        capacity: ENERGY_CAPACITY,
        max_insert: ENERGY_CAPACITY,
        max_extract: ENERGY_CAPACITY,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    });
}

// ---------------------------------------------------------------------------
// The machine tick
// ---------------------------------------------------------------------------

/// One server tick (`native_block_entity_tick`, API 17).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(kind) = Kind::of(&entity.id) else {
        return;
    };
    let facing = facing_of(game, pos).unwrap_or_else(|| kind.default_facing());
    let air_face = facing.opposite();
    let now = game.world.game_time.max(0) as u64;
    let mut air = read_int(&entity.nbt, KEY_AIR);
    let mut energy = read_int(&entity.nbt, KEY_ENERGY).clamp(0, ENERGY_CAPACITY);
    let temperature =
        read_double(&entity.nbt, KEY_TEMPERATURE).unwrap_or(AMBIENT_TEMPERATURE);

    // 1. The air handler's tick: disperse to a lower-pressure connected
    //    machine, then vent what a lone handler cannot keep.
    let neighbour = air_neighbour(game, pos, air_face);
    if let Some(seen) = &neighbour {
        let (mine, theirs) = disperse((air, kind.volume()), (seen.air, seen.kind.volume()));
        air = mine;
        if theirs != seen.air {
            let at = seen.pos;
            let their_kind = seen.kind;
            game.native_with_mod_block_entity(at, |other| {
                if Kind::of(&other.id) == Some(their_kind) {
                    write_int(&mut other.nbt, KEY_AIR, theirs);
                }
            });
        }
    } else if kind == Kind::Flux {
        air -= leak(air, kind.volume());
    }

    // 2. The machine itself, from the pure runtime.
    let run = match kind {
        Kind::Flux => {
            let mut machine = FluxCompressor::with_rates(
                energy,
                air,
                temperature,
                read_double(&entity.nbt, KEY_AIR_RATE).unwrap_or(0.0),
                read_int(&entity.nbt, KEY_FE_RATE),
            );
            machine.set_air_buffer(read_double(&entity.nbt, KEY_AIR_BUFFER).unwrap_or(0.0));
            let run = machine.tick(now);
            air = machine.air;
            energy = machine.energy;
            write_temperature(&mut entity.nbt, machine.heat);
            write_double(&mut entity.nbt, KEY_AIR_RATE, machine.air_per_tick());
            write_int(&mut entity.nbt, KEY_FE_RATE, machine.rf_per_tick());
            write_double(&mut entity.nbt, KEY_AIR_BUFFER, machine.air_buffer());
            run
        }
        Kind::Dynamo => {
            let mut machine = PneumaticDynamo::with_rates(
                energy,
                air,
                temperature,
                read_int(&entity.nbt, KEY_AIR_RATE),
                read_int(&entity.nbt, KEY_FE_RATE),
            );
            let run = machine.tick(now);
            air = machine.air;
            energy = machine.energy;
            write_temperature(&mut entity.nbt, machine.heat);
            write_int(&mut entity.nbt, KEY_AIR_RATE, machine.air_per_tick());
            write_int(&mut entity.nbt, KEY_FE_RATE, machine.rf_per_tick());
            run
        }
    };
    write_int(&mut entity.nbt, KEY_AIR, air);
    write_int(&mut entity.nbt, KEY_ENERGY, energy);

    // 3. The dynamo pushes up to twice its rate into the block the coil faces
    //    (`Capabilities.Energy.BLOCK` both ways, API 142).
    if kind == Kind::Dynamo && run.fe_made > 0 {
        let out = offset(pos, facing);
        let offered = energy.min(run.fe_made.saturating_mul(2));
        let moved = game.native_energy_insert(out, facing.opposite(), offered, false);
        if moved > 0 {
            energy -= moved;
            write_int(&mut entity.nbt, KEY_ENERGY, energy);
        }
    }

    // 4. `active` changes on the machine's own cadence, as upstream writes it.
    if let Some(current) = game.native_block_property(pos, PROP_ACTIVE) {
        let wanted = if run.ran { "true" } else { "false" };
        if current != wanted && now & kind.active_mask() == 0 {
            game.native_set_block_property(pos, PROP_ACTIVE, wanted);
        }
    }

    // `setChanged`: the chunk is saved with what this tick did.
    game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
}

fn write_temperature(nbt: &mut Compound, heat: Heat) {
    write_double(nbt, KEY_TEMPERATURE, heat.temperature);
}

// ---------------------------------------------------------------------------
// The neighbouring air handler
// ---------------------------------------------------------------------------

struct Neighbour {
    pos: [i32; 3],
    air: i32,
    kind: Kind,
}

/// The machine's air handler on `dir`, if a slice-1 machine connects back. The
/// pneumatic face of both machines is `facing.opposite`, as
/// `canConnectPneumatic(side) { side == getRotation().getOpposite() }` says: a
/// neighbour at `dir` reaches us only through its own `facing` side, so its
/// `facing` must be `dir`.
fn air_neighbour(game: &Game<'_>, pos: [i32; 3], dir: Direction) -> Option<Neighbour> {
    let at = offset(pos, dir);
    let entity = match game.world.block_entities.get(&(at[0], at[1], at[2])) {
        Some(BlockEntity::Mod(entity)) => entity,
        _ => return None,
    };
    let kind = Kind::of(&entity.id)?;
    let their_facing = facing_of(game, at)?;
    if their_facing != dir {
        return None;
    }
    Some(Neighbour {
        pos: at,
        air: read_int(&entity.nbt, KEY_AIR),
        kind,
    })
}

fn facing_of(game: &Game<'_>, pos: [i32; 3]) -> Option<Direction> {
    parse_direction(&game.native_block_property(pos, PROP_FACING)?)
}

fn parse_direction(value: &str) -> Option<Direction> {
    match value {
        "down" => Some(Direction::Down),
        "up" => Some(Direction::Up),
        "north" => Some(Direction::North),
        "south" => Some(Direction::South),
        "west" => Some(Direction::West),
        "east" => Some(Direction::East),
        _ => None,
    }
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let [dx, dy, dz] = dir.normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

// ---------------------------------------------------------------------------
// The `/pneumaticcraft` harness controls
// ---------------------------------------------------------------------------

/// The commands (`native_register_commands`, API 8). Upstream has none: they
/// exist so the harness can read and set a machine without a GUI or pipes
/// (precedent: `/powah info|set`, `/charginggadgets energy`).
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("pneumaticcraft info <pos:block_pos>"));
    commands.push(NativeCommand::op(
        "pneumaticcraft air <pos:block_pos> <amount:integer>",
        2,
    ));
    commands.push(NativeCommand::op(
        "pneumaticcraft energy <pos:block_pos> <amount:integer>",
        2,
    ));
}

/// `/pneumaticcraft info|air|energy` (`native_perform_command`, API 8).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_perform_command(
    game: &mut Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    if words.first() != Some(&"pneumaticcraft") {
        return;
    }
    *ran = true;
    match words.get(1).copied() {
        Some("info") => {
            let Some(pos) = parse_pos(&words[2..]) else {
                game.native_feedback(Text::literal("usage: /pneumaticcraft info <x> <y> <z>"));
                return;
            };
            game.native_feedback(Text::literal(readout(game, pos)));
        }
        Some("air") => {
            let Some((pos, amount)) = parse_pos_amount(&words[2..]) else {
                game.native_feedback(Text::literal(
                    "usage: /pneumaticcraft air <x> <y> <z> <mL>",
                ));
                return;
            };
            let changed = game
                .native_with_mod_block_entity(pos, |entity| {
                    let Some(kind) = Kind::of(&entity.id) else {
                        return false;
                    };
                    let temperature = read_double(&entity.nbt, KEY_TEMPERATURE)
                        .unwrap_or(AMBIENT_TEMPERATURE);
                    let clamped = match kind {
                        Kind::Flux => {
                            let mut machine = FluxCompressor::new(0, 0, temperature);
                            machine.set_air(amount);
                            machine.air
                        }
                        Kind::Dynamo => {
                            let mut machine = PneumaticDynamo::new(0, 0, temperature);
                            machine.set_air(amount);
                            machine.air
                        }
                    };
                    write_int(&mut entity.nbt, KEY_AIR, clamped);
                    true
                })
                .unwrap_or(false);
            game.native_feedback(Text::literal(if changed {
                format!("PneumaticCraft at {pos:?}: air set to {amount} mL.")
            } else {
                format!("{pos:?} is not a slice-1 PneumaticCraft machine.")
            }));
        }
        Some("energy") => {
            let Some((pos, amount)) = parse_pos_amount(&words[2..]) else {
                game.native_feedback(Text::literal(
                    "usage: /pneumaticcraft energy <x> <y> <z> <FE>",
                ));
                return;
            };
            let changed = game
                .native_with_mod_block_entity(pos, |entity| {
                    if Kind::of(&entity.id).is_none() {
                        return false;
                    }
                    write_int(&mut entity.nbt, KEY_ENERGY, amount.clamp(0, ENERGY_CAPACITY));
                    true
                })
                .unwrap_or(false);
            game.native_feedback(Text::literal(if changed {
                format!("PneumaticCraft at {pos:?}: FE set to {amount}.")
            } else {
                format!("{pos:?} is not a slice-1 PneumaticCraft machine.")
            }));
        }
        _ => game.native_feedback(Text::literal(
            "usage: /pneumaticcraft info|air|energy ...",
        )),
    }
}

/// `/pneumaticcraft info`: one line the harness parses.
fn readout(game: &Game<'_>, pos: [i32; 3]) -> String {
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
    else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let Some(kind) = Kind::of(&entity.id) else {
        return format!("{pos:?} is a {} not a PneumaticCraft machine.", entity.id);
    };
    let air = read_int(&entity.nbt, KEY_AIR);
    let energy = read_int(&entity.nbt, KEY_ENERGY).clamp(0, ENERGY_CAPACITY);
    let volume = kind.volume();
    let temperature =
        read_double(&entity.nbt, KEY_TEMPERATURE).unwrap_or(AMBIENT_TEMPERATURE);
    let efficiency = heat_efficiency(temperature);
    let active = game
        .native_block_property(pos, PROP_ACTIVE)
        .unwrap_or_else(|| "false".to_owned());
    format!(
        "PC {} {pos:?} air={} volume={} bar={:.3} fe={} temp={:.1} eff={} active={} t={}",
        entity.id,
        air,
        volume,
        pressure(air, volume),
        energy,
        temperature,
        efficiency,
        active,
        game.world.game_time
    )
}

fn parse_pos(words: &[&str]) -> Option<[i32; 3]> {
    Some([
        words.first()?.parse().ok()?,
        words.get(1)?.parse().ok()?,
        words.get(2)?.parse().ok()?,
    ])
}

fn parse_pos_amount(words: &[&str]) -> Option<([i32; 3], i32)> {
    let pos = parse_pos(words)?;
    let amount = words.get(3)?.parse().ok()?;
    Some((pos, amount))
}

// ---------------------------------------------------------------------------
// Saved-compound reads
// ---------------------------------------------------------------------------

fn read_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(JavaString::from(key), Tag::Int(value));
}

fn read_double(nbt: &Compound, key: &str) -> Option<f64> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Double(bits)) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

fn write_double(nbt: &mut Compound, key: &str, value: f64) {
    nbt.insert(JavaString::from(key), Tag::Double(value.to_bits()));
}
