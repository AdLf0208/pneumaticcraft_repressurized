//! The slice's glue: the two block entity types and their declarations, the machine tick, the
//! neighbouring air handler and the `/pneumaticcraft` harness commands.
//!
//! Every rule lives in [`crate::runtime`] (the retired bundle's `runtime.rs`, byte-identical);
//! this file reads the game, calls it and writes the result back. Its tests pin what it
//! *declares* (the two 100,000 FE buffers), the saved compound's keys and types, the facing rule
//! that joins two machines, the `active` cadence, and the exact line `/pneumaticcraft info`
//! prints (the 0.1.0 e2e parses it field by field).
//!
//! The retired `.hnmod` bundle read the game through `Game` and wrote a neighbour's compound
//! directly with `native_with_mod_block_entity`. Here a block entity's saved data is the
//! checked view the tick is handed (`event.entity.nbt`) for its **own** machine, and a handle
//! (`host.block_entity(level, pos)` + `merge_data`) for a neighbour's: a ticked block entity is
//! out of the world for its own tick, so its own payload can only be written through the event.
use mod_api::{
    self, BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Command, CommandArgumentValue, Compound, Direction, ENERGY,
    Flow, Host, HostBlockStates, HostTransaction, LevelRef, LevelSide, ModEnergyBuffer, Registrar,
    RegistrationContext, RunModCommandEvent, ServerHost, Tag, Text,
};

use crate::runtime::{
    AMBIENT_TEMPERATURE, ENERGY_CAPACITY, FluxCompressor, PneumaticDynamo, VOLUME_FLUX_COMPRESSOR,
    VOLUME_PNEUMATIC_DYNAMO, disperse, heat_efficiency, leak, pressure,
};

/// The flux compressor block and its item (`ModBlocks.FLUX_COMPRESSOR`).
pub(crate) const FLUX: &str = "pneumaticcraft:flux_compressor";
/// The pneumatic dynamo block and its item (`ModBlocks.PNEUMATIC_DYNAMO`).
pub(crate) const DYNAMO: &str = "pneumaticcraft:pneumatic_dynamo";

/// The flux compressor's block entity type, in this mod's namespace.
const FLUX_TYPE: &str = "flux_compressor";
/// The pneumatic dynamo's block entity type.
const DYNAMO_TYPE: &str = "pneumatic_dynamo";

/// The air handler's saved amount (`AirHandler.Air` upstream).
const KEY_AIR: &str = "air";
/// The FE buffer's amount: the int the engine's energy capability reads for a mod block.
const KEY_ENERGY: &str = "energy";
/// The heat exchanger's saved temperature, kelvin.
const KEY_TEMPERATURE: &str = "temperature";
/// The flux compressor's last 5-tick air rate (upstream's transient `airPerTick` field, kept so
/// the rate survives between ticks); the dynamo's last 20-tick one, as an int.
const KEY_AIR_RATE: &str = "air_rate";
/// The machine's last rate in FE a tick (`rfPerTick`).
const KEY_FE_RATE: &str = "fe_rate";
/// The whole-millibucket carry (upstream's transient `airBuffer`).
const KEY_AIR_BUFFER: &str = "air_buffer";
/// Upstream's block state (`PNCBlockStateProperties.ACTIVE`).
const PROP_ACTIVE: &str = "active";
/// Upstream's `directionProperty()`.
const PROP_FACING: &str = "facing";

/// Which slice-1 machine a block or a part belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Flux,
    Dynamo,
}

impl Kind {
    /// The machine a block id is, or `None` for anything else.
    pub(crate) fn of(id: &str) -> Option<Self> {
        match id {
            FLUX => Some(Self::Flux),
            DYNAMO => Some(Self::Dynamo),
            _ => None,
        }
    }

    /// The air handler's volume (`PneumaticValues.VOLUME_*`).
    pub(crate) fn volume(self) -> i32 {
        match self {
            Self::Flux => VOLUME_FLUX_COMPRESSOR,
            Self::Dynamo => VOLUME_PNEUMATIC_DYNAMO,
        }
    }

    /// The facing a machine has before anything sets the property: the block's default state.
    /// Both blocks declare `facing` with `north` as the default; a dynamo's coil is placed
    /// against the clicked face, and `north` is where 0.1.0's harness always saw it start.
    pub(crate) fn default_facing(self) -> Direction {
        match self {
            Self::Flux => Direction::North,
            Self::Dynamo => Direction::Up,
        }
    }

    /// Upstream's `ACTIVE` refresh cadence: `& 0x7` and `& 0xf`.
    pub(crate) fn active_mask(self) -> u64 {
        match self {
            Self::Flux => 0x7,
            Self::Dynamo => 0xf,
        }
    }
}

// ---------------------------------------------------------------------------
// Declarations (registration phase)
// ---------------------------------------------------------------------------

/// Declare both machines: a block entity type each, its FE buffer, and the Part that ticks it.
///
/// A declaration is keyed by the block, not written into a shared output, so the retired
/// `block_entity_energy` hook's "leave a foreign block's entry alone" hazard cannot recur.
pub(crate) fn declare(registrar: &mut dyn Registrar, context: &RegistrationContext) {
    let flux = registrar.block_key(FLUX);
    registrar.block_energy(&flux, battery());
    let dynamo = registrar.block_key(DYNAMO);
    registrar.block_energy(&dynamo, battery());

    let flux_type = registrar
        .block_entity_type(FLUX_TYPE)
        .blocks(&[FLUX_TYPE])
        .build();
    let dynamo_type = registrar
        .block_entity_type(DYNAMO_TYPE)
        .blocks(&[DYNAMO_TYPE])
        .build();
    if let Err(error) = registrar.block_entity_part(&flux_type, FluxPart) {
        context.refuse(&error.to_string());
    }
    if let Err(error) = registrar.block_entity_part(&dynamo_type, DynamoPart) {
        context.refuse(&error.to_string());
    }
}

/// Both machines' FE buffer (`native_block_entity_energy`, API 142): 100,000 FE, filled and
/// drained from any face.
///
/// Upstream reaches the buffer only through `getEnergyHandler(dir)` — the flux compressor on
/// every face but its `facing`, the dynamo on `facing` alone. A declaration is keyed by the
/// block id, not by a block state, so it cannot express the two machines' face sets; both take
/// all six, which is only wider than upstream. Recorded in `notes.md`.
pub(crate) fn battery() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: ENERGY_CAPACITY,
        max_insert: ENERGY_CAPACITY,
        max_extract: ENERGY_CAPACITY,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    }
}

// ---------------------------------------------------------------------------
// The Parts
// ---------------------------------------------------------------------------

/// The flux compressor's behaviour.
pub(crate) struct FluxPart;

#[mod_api::horizon_part]
impl BlockPart for FluxPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for FluxPart {
    /// `EntityBlock.newBlockEntity`: a fresh machine starts empty. The declared type gives the
    /// same empty compound; this states it, as the retired `block_entity` hook did.
    fn block_entity(
        &self,
        _level: LevelSide,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    /// `EntityBlock.getTicker`: it ticks. Never clear a value an earlier mod set.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick of a flux compressor.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        tick(host, event, Kind::Flux);
    }
}

/// The pneumatic dynamo's behaviour.
pub(crate) struct DynamoPart;

#[mod_api::horizon_part]
impl BlockPart for DynamoPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for DynamoPart {
    /// `EntityBlock.newBlockEntity`: a fresh machine starts empty.
    fn block_entity(
        &self,
        _level: LevelSide,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    /// `EntityBlock.getTicker`: it ticks on the server.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick of a pneumatic dynamo.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        tick(host, event, Kind::Dynamo);
    }
}

// ---------------------------------------------------------------------------
// The machine tick
// ---------------------------------------------------------------------------

/// One server tick (`native_block_entity_tick`, API 17) of the machine `kind`.
///
/// The order is upstream's: the air handler's tick (disperse to a lower-pressure connected
/// machine, then vent what a lone handler cannot keep), the machine itself, the dynamo's push
/// into the block its coil faces, then the block state.
pub(crate) fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>, kind: Kind) {
    let mut saved = std::mem::take(&mut event.entity.nbt);
    let before = saved.clone();
    let Some(level) = level_in_place(host) else {
        event.entity.nbt = saved;
        return;
    };
    let pos = event.pos;
    let now = level.game_time(&*host).unwrap_or(0).max(0) as u64;
    let state = level.block_state(&*host, pos);
    let facing = state
        .and_then(|state| property(&*host, state, PROP_FACING))
        .and_then(|value| parse_direction(&value))
        .unwrap_or_else(|| kind.default_facing());
    let active_now = state.and_then(|state| property(&*host, state, PROP_ACTIVE));
    let air_face = facing.opposite();

    let mut air = read_int(&saved, KEY_AIR);
    let mut energy = read_int(&saved, KEY_ENERGY).clamp(0, ENERGY_CAPACITY);
    let temperature = read_double(&saved, KEY_TEMPERATURE).unwrap_or(AMBIENT_TEMPERATURE);

    // 1. The air handler's tick: disperse to a lower-pressure connected machine, then vent
    //    what a lone handler cannot keep.
    match air_neighbour(&*host, level, pos, air_face) {
        Some(seen) => {
            let (mine, theirs) = disperse((air, kind.volume()), (seen.air, seen.kind.volume()));
            air = mine;
            if theirs != seen.air {
                let mut patch = Compound::new();
                write_int(&mut patch, KEY_AIR, theirs);
                if let Some(other) = host.block_entity(level, seen.pos) {
                    let _ = other.merge_data(host, patch);
                }
            }
        }
        None => {
            if kind == Kind::Flux {
                air -= leak(air, kind.volume());
            }
        }
    }

    // 2. The machine itself, from the pure runtime.
    let run = match kind {
        Kind::Flux => {
            let mut machine = FluxCompressor::with_rates(
                energy,
                air,
                temperature,
                read_double(&saved, KEY_AIR_RATE).unwrap_or(0.0),
                read_int(&saved, KEY_FE_RATE),
            );
            machine.set_air_buffer(read_double(&saved, KEY_AIR_BUFFER).unwrap_or(0.0));
            let run = machine.tick(now);
            air = machine.air;
            energy = machine.energy;
            write_double(&mut saved, KEY_TEMPERATURE, machine.heat.temperature);
            write_double(&mut saved, KEY_AIR_RATE, machine.air_per_tick());
            write_int(&mut saved, KEY_FE_RATE, machine.rf_per_tick());
            write_double(&mut saved, KEY_AIR_BUFFER, machine.air_buffer());
            run
        }
        Kind::Dynamo => {
            let mut machine = PneumaticDynamo::with_rates(
                energy,
                air,
                temperature,
                read_int(&saved, KEY_AIR_RATE),
                read_int(&saved, KEY_FE_RATE),
            );
            let run = machine.tick(now);
            air = machine.air;
            energy = machine.energy;
            write_double(&mut saved, KEY_TEMPERATURE, machine.heat.temperature);
            write_int(&mut saved, KEY_AIR_RATE, machine.air_per_tick());
            write_int(&mut saved, KEY_FE_RATE, machine.rf_per_tick());
            run
        }
    };

    // 3. The dynamo pushes up to twice its rate into the block the coil faces, through the
    //    engine's own energy capability on that block (upstream's `Capabilities.Energy.BLOCK`
    //    both ways). Its own buffer is written here: a block entity out of the world for its
    //    tick answers no capability, so it cannot take its FE back through one.
    if kind == Kind::Dynamo && run.fe_made > 0 {
        let out = offset(pos, facing);
        let offered = energy.min(run.fe_made.saturating_mul(2));
        if offered > 0
            && let Some(target) = level.capability(host, &ENERGY, out, Some(facing.opposite()))
        {
            let asked = offered as u32;
            let moved = host
                .transaction(|host, tx| Ok::<u32, ()>(target.insert(host, tx, asked)))
                .unwrap_or(0)
                .min(asked);
            if moved > 0 {
                energy -= moved as i32;
            }
        }
    }

    write_int(&mut saved, KEY_AIR, air);
    write_int(&mut saved, KEY_ENERGY, energy);

    // 4. `active` changes on the machine's own cadence, as upstream writes it.
    let wanted = if run.ran { "true" } else { "false" };
    let mut touched = saved != before;
    if active_now.as_deref() != Some(wanted) && now & kind.active_mask() == 0 {
        host.set_block_property(pos, PROP_ACTIVE, wanted);
        touched = true;
    }
    event.entity.nbt = saved;
    // An idle machine leaves the chunk alone: the retired bundle marked it changed every tick.
    if !touched {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// The machine's air handler on `dir`, if a slice-1 machine connects back.
///
/// The pneumatic face of both machines is `facing.opposite`, as
/// `canConnectPneumatic(side) { side == getRotation().getOpposite() }` says: a neighbour at
/// `dir` reaches us only through its own pneumatic face, so its `facing` must be `dir`.
fn air_neighbour(
    host: &dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    dir: Direction,
) -> Option<Neighbour> {
    let at = offset(pos, dir);
    let kind = kind_at(host, level, at)?;
    let state = level.block_state(host, at)?;
    let their_facing =
        property(host, state, PROP_FACING).and_then(|value| parse_direction(&value))?;
    if their_facing != dir {
        return None;
    }
    let entity = host.block_entity(level, at)?;
    let data = entity.data(host)?;
    Some(Neighbour {
        pos: at,
        air: read_int(&data, KEY_AIR),
        kind,
    })
}

/// A connected neighbour, read from its own saved compound.
struct Neighbour {
    pos: [i32; 3],
    air: i32,
    kind: Kind,
}

/// Which slice-1 machine stands at `pos`, if one does.
fn kind_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Kind> {
    let block = level.block(host, pos)?;
    Kind::of(&block.id(host))
}

/// The level the server is ticking now: the one a block-entity tick runs in.
fn level_in_place(host: &dyn ServerHost) -> Option<LevelRef> {
    host.levels()
        .list_levels()
        .into_iter()
        .find(|&level| level.in_place(host))
}

/// A block state property's value, as `/setblock` writes it.
fn property(host: &dyn Host, state: mod_api::BlockState, name: &str) -> Option<String> {
    let facts = host.block_states();
    state
        .property(&facts, name)
        .ok()
        .flatten()
        .map(str::to_owned)
}

/// A block state's `facing` value as a [`Direction`].
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

/// The commands (`native_register_commands`, API 8). Upstream has none: they exist so the
/// harness can read and set a machine without a GUI or a pressure network (precedent
/// `/powah info|set`, `/charginggadgets energy`).
pub(crate) fn declare_commands(commands: &mut Vec<Command>) {
    commands.push(Command::new("pneumaticcraft info <pos:block_pos>"));
    commands.push(Command::op(
        "pneumaticcraft air <pos:block_pos> <amount:integer>",
        2,
    ));
    commands.push(Command::op(
        "pneumaticcraft energy <pos:block_pos> <amount:integer>",
        2,
    ));
}

/// `/pneumaticcraft info|air|energy` (`native_perform_command`, API 8).
pub(crate) fn run_command(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    let literals: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
    let pos = block_pos(event.command.argument("pos"));
    let amount = match event.command.argument("amount") {
        Some(CommandArgumentValue::Integer(amount)) => Some(*amount),
        _ => None,
    };
    let line = match (literals.as_slice(), pos, amount) {
        (["pneumaticcraft", "info"], Some(pos), _) => readout(host, pos),
        (["pneumaticcraft", "air"], Some(pos), Some(amount)) => match set_air(host, pos, amount) {
            // The machine clamps at the critical pressure, so the reply states what it stored,
            // not what was asked for (the 0.1.0 line repeated the request).
            Some(stored) => format!("PneumaticCraft at {pos:?}: air set to {stored} mL."),
            None => format!("{pos:?} is not a slice-1 PneumaticCraft machine."),
        },
        (["pneumaticcraft", "energy"], Some(pos), Some(amount)) => {
            match set_energy(host, pos, amount) {
                Some(stored) => format!("PneumaticCraft at {pos:?}: FE set to {stored}."),
                None => format!("{pos:?} is not a slice-1 PneumaticCraft machine."),
            }
        }
        _ => return Flow::Continue,
    };
    let _ = host.commands().feedback(Text::literal(line));
    Flow::Cancel
}

/// The command's `<pos:block_pos>` argument.
fn block_pos(argument: Option<&CommandArgumentValue>) -> Option<[i32; 3]> {
    match argument? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

/// What `/pneumaticcraft info` prints about one machine.
pub(crate) struct Sight<'a> {
    pub(crate) id: &'a str,
    pub(crate) pos: [i32; 3],
    pub(crate) air: i32,
    pub(crate) volume: i32,
    pub(crate) energy: i32,
    pub(crate) temperature: f64,
    pub(crate) active: &'a str,
    pub(crate) now: i64,
}

impl Sight<'_> {
    /// The report line, one field per number the model owns: the 0.1.0 e2e parses every one.
    pub(crate) fn line(&self) -> String {
        format!(
            "PC {} {:?} air={} volume={} bar={:.3} fe={} temp={:.1} eff={} active={} t={}",
            self.id,
            self.pos,
            self.air,
            self.volume,
            pressure(self.air, self.volume),
            self.energy,
            self.temperature,
            heat_efficiency(self.temperature),
            self.active,
            self.now
        )
    }
}

/// `/pneumaticcraft info <pos>`: one line the harness parses.
fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(level) = level_in_place(host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let Some(block) = level.block(&*host, pos) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let id = block.id(&*host);
    let Some(kind) = Kind::of(&id) else {
        return format!("{pos:?} is a {id} not a PneumaticCraft machine.");
    };
    let data = entity.data(&*host).unwrap_or_default();
    let state = level.block_state(&*host, pos);
    let active = state
        .and_then(|state| property(&*host, state, PROP_ACTIVE))
        .unwrap_or_else(|| "false".to_owned());
    let sight = Sight {
        id: &id,
        pos,
        air: read_int(&data, KEY_AIR),
        volume: kind.volume(),
        energy: read_int(&data, KEY_ENERGY).clamp(0, ENERGY_CAPACITY),
        temperature: read_double(&data, KEY_TEMPERATURE).unwrap_or(AMBIENT_TEMPERATURE),
        active: &active,
        now: level.game_time(&*host).unwrap_or(0),
    };
    sight.line()
}

/// `/pneumaticcraft air <pos> <mL>`: set the handler's amount through the same clamp the model
/// owns (never above the critical pressure). Answers what the machine stores.
fn set_air(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) -> Option<i32> {
    let level = level_in_place(host)?;
    let kind = kind_at(&*host, level, pos)?;
    let entity = host.block_entity(level, pos)?;
    let data = entity.data(&*host).unwrap_or_default();
    let temperature = read_double(&data, KEY_TEMPERATURE).unwrap_or(AMBIENT_TEMPERATURE);
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
    let mut patch = Compound::new();
    write_int(&mut patch, KEY_AIR, clamped);
    // `merge_data` is `/data merge block`: it loads the patch and marks the chunk changed,
    // which is the `setChanged` the retired bundle had to fake.
    entity.merge_data(host, patch).ok().map(|_| clamped)
}

/// `/pneumaticcraft energy <pos> <FE>`: set the buffer's amount, clamped to its capacity.
fn set_energy(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) -> Option<i32> {
    let level = level_in_place(host)?;
    kind_at(&*host, level, pos)?;
    let entity = host.block_entity(level, pos)?;
    let stored = amount.clamp(0, ENERGY_CAPACITY);
    let mut patch = Compound::new();
    write_int(&mut patch, KEY_ENERGY, stored);
    entity.merge_data(host, patch).ok().map(|_| stored)
}

// ---------------------------------------------------------------------------
// Saved-compound reads
// ---------------------------------------------------------------------------

fn read_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

/// A saved double, kept as its IEEE bits (`Tag::Double`); `None` when absent or another type.
fn read_double(nbt: &Compound, key: &str) -> Option<f64> {
    match nbt.get(&key.into()) {
        Some(Tag::Double(bits)) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

fn write_double(nbt: &mut Compound, key: &str, value: f64) {
    nbt.insert(key.into(), Tag::Double(value.to_bits()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{VOLUME_FLUX_COMPRESSOR, VOLUME_PNEUMATIC_DYNAMO};

    #[test]
    fn the_two_machines_declare_the_same_hundred_thousand_fe_buffer() {
        let buffer = battery();
        assert_eq!(buffer.capacity, ENERGY_CAPACITY, "100,000 FE each");
        assert_eq!(buffer.max_insert, ENERGY_CAPACITY);
        assert_eq!(buffer.max_extract, ENERGY_CAPACITY);
        assert_eq!(
            buffer.insert_faces, [true; 6],
            "a declaration is keyed by the block, not the state, so all six faces"
        );
        assert_eq!(buffer.extract_faces, [true; 6]);
    }

    #[test]
    fn the_two_machines_have_their_own_volumes() {
        assert_eq!(Kind::Flux.volume(), VOLUME_FLUX_COMPRESSOR);
        assert_eq!(Kind::Dynamo.volume(), VOLUME_PNEUMATIC_DYNAMO);
        assert_eq!(Kind::of(FLUX), Some(Kind::Flux));
        assert_eq!(Kind::of(DYNAMO), Some(Kind::Dynamo));
        assert_eq!(Kind::of("minecraft:stone"), None);
    }

    #[test]
    fn the_pneumatic_face_is_behind_the_coil() {
        assert_eq!(Kind::Flux.default_facing(), Direction::North);
        assert_eq!(Kind::Dynamo.default_facing(), Direction::Up);
        assert_eq!(Direction::East.opposite(), Direction::West);
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        for (value, direction) in [
            ("down", Direction::Down),
            ("up", Direction::Up),
            ("north", Direction::North),
            ("south", Direction::South),
            ("west", Direction::West),
            ("east", Direction::East),
        ] {
            assert_eq!(parse_direction(value), Some(direction), "{value}");
        }
        assert_eq!(parse_direction("sideways"), None);
    }

    #[test]
    fn the_active_cadence_is_upstreams() {
        assert_eq!(Kind::Flux.active_mask(), 0x7, "FluxCompressorBlockEntity");
        assert_eq!(
            Kind::Dynamo.active_mask(),
            0xf,
            "PneumaticDynamoBlockEntity"
        );
    }

    #[test]
    fn the_saved_compound_round_trips_its_keys_and_types() {
        let mut nbt = Compound::new();
        assert_eq!(read_int(&nbt, KEY_AIR), 0, "an absent key reads 0");
        assert_eq!(read_double(&nbt, KEY_TEMPERATURE), None);
        write_int(&mut nbt, KEY_AIR, 1_600);
        write_double(&mut nbt, KEY_TEMPERATURE, 300.875);
        assert_eq!(read_int(&nbt, KEY_AIR), 1_600);
        assert_eq!(read_double(&nbt, KEY_TEMPERATURE), Some(300.875));
        assert_eq!(
            nbt.get(&KEY_AIR.into()),
            Some(&Tag::Int(1_600)),
            "the energy capability and the air amount are ints, as 0.1.0 wrote them"
        );
        assert_eq!(
            nbt.get(&KEY_TEMPERATURE.into()),
            Some(&Tag::Double(300.875f64.to_bits())),
            "the heat exchanger is a double"
        );
        assert_eq!(
            read_int(&nbt, KEY_TEMPERATURE),
            0,
            "a double read as an int is 0, never a wrong number"
        );
    }

    #[test]
    fn a_fresh_machine_reads_empty_at_ambient() {
        let nbt = Compound::new();
        assert_eq!(read_int(&nbt, KEY_AIR), 0);
        assert_eq!(read_int(&nbt, KEY_ENERGY), 0);
        assert_eq!(
            read_double(&nbt, KEY_TEMPERATURE).unwrap_or(AMBIENT_TEMPERATURE),
            AMBIENT_TEMPERATURE
        );
    }

    #[test]
    fn the_info_line_is_byte_identical_to_the_one_the_e2e_parses() {
        let sight = Sight {
            id: FLUX,
            pos: [302, 101, 0],
            air: 1_600,
            volume: VOLUME_FLUX_COMPRESSOR,
            energy: 96_000,
            temperature: 297.2,
            active: "true",
            now: 1_240,
        };
        assert_eq!(
            sight.line(),
            "PC pneumaticcraft:flux_compressor [302, 101, 0] air=1600 volume=10000 bar=0.160 \
             fe=96000 temp=297.2 eff=100 active=true t=1240"
        );
        let idle = Sight {
            id: DYNAMO,
            pos: [-8, 64, 12],
            air: 0,
            volume: VOLUME_PNEUMATIC_DYNAMO,
            energy: 0,
            temperature: 300.0,
            active: "false",
            now: 0,
        };
        assert_eq!(
            idle.line(),
            "PC pneumaticcraft:pneumatic_dynamo [-8, 64, 12] air=0 volume=10000 bar=0.000 \
             fe=0 temp=300.0 eff=100 active=false t=0"
        );
    }
}
