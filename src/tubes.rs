//! Slice 2: the pressure-tube network's pure rules.
//!
//! `runtime.rs` is slice 1's file and stays byte-identical; the tube family's arithmetic lives
//! here so each slice's pure part is one small, tested module. No game types: the glue maps
//! `mod_api::Direction` to [`Side`] and back.
//!
//! Upstream (PneumaticCraft: Repressurized, `pnc-repressurized` `1.21` @ `7bcbcbc7`, read in
//! `PressureTubeBlockEntity`, `MachineAirHandler`, `PneumaticValues` and `PressureTier`; see
//! `FINDINGS.md` and `PLAN-SLICE2.md`):
//!
//! * every tube is its own air handler — 1,000 mL (basic and reinforced) or 4,000 mL (advanced);
//! * a handler sends air to each **strictly lower-pressure** connected neighbour, to that
//!   neighbour's share of the total air by volume, never backwards;
//! * a tube vents `(int)(pressure * 40) + 20` mL a tick only when **exactly one** side has an
//!   air-handler neighbour, so a run holds air and a dead end empties;
//! * a tube with a single connection also *shows* the opposite side, so a run looks like a run.

use crate::runtime::{CRITICAL_PRESSURE, leak, pressure};

/// `PneumaticValues.VOLUME_PRESSURE_TUBE`: the basic and the reinforced tube.
pub const VOLUME_PRESSURE_TUBE: i32 = 1_000;
/// `PneumaticValues.VOLUME_ADVANCED_PRESSURE_TUBE`.
pub const VOLUME_ADVANCED_PRESSURE_TUBE: i32 = 4_000;

/// One of a block's six sides, in Minecraft's own order (`Direction.get3DDataValue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Side {
    /// All six, in Minecraft's order.
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    /// The side's name, as a block state property is spelled.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
        }
    }

    /// The side a tube would vent from when this one is its only connection
    /// (upstream's `PressureTubeBlockEntity.getLeakDir`): the open face opposite it.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    /// `DirectionUtil.getDirectionBit`: `1 << get3DDataValue`.
    #[must_use]
    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A set of sides (`PressureTubeBlockEntity`'s `sides…Connected` ints, typed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sides(u8);

impl Sides {
    /// No side.
    pub const NONE: Self = Self(0);

    /// Every side.
    #[must_use]
    pub const fn all() -> Self {
        Self(0b11_1111)
    }

    /// The set holding exactly `sides`.
    #[must_use]
    pub fn of(sides: impl IntoIterator<Item = Side>) -> Self {
        let mut set = Self::NONE;
        for side in sides {
            set.insert(side);
        }
        set
    }

    /// Adds one side.
    pub fn insert(&mut self, side: Side) {
        self.0 |= side.bit();
    }

    /// Whether the set holds `side`.
    #[must_use]
    pub const fn has(self, side: Side) -> bool {
        self.0 & side.bit() != 0
    }

    /// How many sides the set holds.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set holds nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The one side the set holds, or `None` when it holds none or several.
    #[must_use]
    pub fn the_only(self) -> Option<Side> {
        (self.count() == 1)
            .then(|| Side::ALL.into_iter().find(|side| self.has(*side)))
            .flatten()
    }

    /// Both sets' sides.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The sides in Minecraft's order.
    pub fn iter(self) -> impl Iterator<Item = Side> {
        Side::ALL.into_iter().filter(move |side| self.has(*side))
    }
}

/// Which tube a block id is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tube {
    Pressure,
    Reinforced,
    Advanced,
}

impl Tube {
    /// The block id (`ModBlocks`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pressure => "pneumaticcraft:pressure_tube",
            Self::Reinforced => "pneumaticcraft:reinforced_pressure_tube",
            Self::Advanced => "pneumaticcraft:advanced_pressure_tube",
        }
    }

    /// The tube a block id is, or `None` for anything else.
    #[must_use]
    pub fn of(id: &str) -> Option<Self> {
        [Self::Pressure, Self::Reinforced, Self::Advanced]
            .into_iter()
            .find(|tube| tube.id() == id)
    }

    /// Its air handler's volume (`PneumaticValues.VOLUME_*`).
    #[must_use]
    pub const fn volume(self) -> i32 {
        match self {
            Self::Pressure | Self::Reinforced => VOLUME_PRESSURE_TUBE,
            Self::Advanced => VOLUME_ADVANCED_PRESSURE_TUBE,
        }
    }

    /// `PressureTier.getDangerPressure`: the tier the tube is made of.
    #[must_use]
    pub const fn danger_pressure(self) -> f32 {
        match self {
            Self::Pressure => 5.0,
            Self::Reinforced => 10.0,
            Self::Advanced => 20.0,
        }
    }

    /// `PressureTier.getCriticalPressure`: where the harness setter stops.
    #[must_use]
    pub const fn critical_pressure(self) -> f32 {
        match self {
            Self::Pressure => 7.0,
            Self::Reinforced => 12.0,
            Self::Advanced => CRITICAL_PRESSURE,
        }
    }

    /// The `/pneumaticcraft air` setter's clamp: never above the critical pressure.
    #[must_use]
    pub fn clamped_air(self, air: i32) -> i32 {
        air.clamp(0, (self.critical_pressure() * self.volume() as f32) as i32)
    }
}

/// One air handler as the network sees it: its air and its volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handler {
    /// Millibuckets held.
    pub air: i32,
    /// The handler's volume in millibuckets.
    pub volume: i32,
}

impl Handler {
    /// A handler holding `air` mL in `volume` mL.
    #[must_use]
    pub const fn new(air: i32, volume: i32) -> Self {
        Self { air, volume }
    }

    /// `BasicAirHandler.getPressure`: air over volume.
    #[must_use]
    pub fn pressure(self) -> f32 {
        pressure(self.air, self.volume)
    }
}

/// `MachineAirHandler.disperseAir` in the tube network: one handler and every connected
/// neighbour, in the order they were found.
///
/// Only neighbours at a strictly lower pressure receive anything, and only those neighbours take
/// part in the total, so air never flows uphill. Each such neighbour is brought to
/// `total_air * neighbour_volume / total_volume` — its share of the network's air by volume — and
/// the sender keeps the rest.
///
/// Answers the sender's air after the tick and, in the same order as `neighbours`, how much each
/// neighbour received (0 for one that was not lower-pressure).
#[must_use]
pub fn disperse_to_lower(mine: Handler, neighbours: &[Handler]) -> (i32, Vec<i32>) {
    let mut moved = vec![0; neighbours.len()];
    if mine.volume <= 0 {
        return (mine.air, moved);
    }
    let share: Vec<usize> = neighbours
        .iter()
        .enumerate()
        .filter(|(_, other)| {
            other.volume > 0 && pressure(other.air, other.volume) < mine.pressure()
        })
        .map(|(index, _)| index)
        .collect();
    if share.is_empty() {
        return (mine.air, moved);
    }
    // Upstream totals only the sender and the neighbours slower than it, so a fuller
    // neighbour can never pull air out of this one.
    let mut total_air = i64::from(mine.air);
    let mut total_volume = i64::from(mine.volume);
    for index in &share {
        total_air += i64::from(neighbours[*index].air);
        total_volume += i64::from(neighbours[*index].volume);
    }
    let mut air = mine.air;
    for index in share {
        let Some(other) = neighbours.get(index) else {
            continue;
        };
        let target = (total_air * i64::from(other.volume) / total_volume) as i32;
        let send = (target - other.air).max(0).min(air);
        moved[index] = send;
        air -= send;
    }
    (air, moved)
}

/// Whether a tube vents this tick: exactly one side reaches an air handler and no side is closed
/// (`PressureTubeBlockEntity.tickServer`, `getLeakDir`). A closed side or a module stops it; this
/// slice has neither, so the closed set is always empty.
#[must_use]
pub fn tube_leaks(connected: Sides, closed: Sides) -> bool {
    closed.is_empty() && connected.the_only().is_some()
}

/// `PressureTubeBlockEntity.calculateVisuallyConnected`: a tube with one real connection and
/// nothing closed or attached also shows the opposite side, so the end of a run looks like a run.
#[must_use]
pub fn visual_sides(connected: Sides, closed: Sides) -> Sides {
    if !closed.is_empty() {
        return connected;
    }
    match connected.the_only() {
        Some(side) => connected.union(Sides::of([side.opposite()])),
        None => connected,
    }
}

/// What a tube vents in one tick, capped at what it holds (`MachineAirHandler.handleAirLeak`).
#[must_use]
pub fn tube_leak(air: i32, volume: i32) -> i32 {
    leak(air, volume)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::disperse;

    #[test]
    fn the_three_tubes_have_upstreams_volumes_and_tiers() {
        assert_eq!(Tube::Pressure.volume(), 1_000, "PneumaticValues");
        assert_eq!(Tube::Reinforced.volume(), 1_000);
        assert_eq!(Tube::Advanced.volume(), 4_000);
        assert_eq!(Tube::Pressure.danger_pressure(), 5.0, "TIER_ONE");
        assert_eq!(Tube::Pressure.critical_pressure(), 7.0);
        assert_eq!(Tube::Reinforced.danger_pressure(), 10.0, "TIER_ONE_HALF");
        assert_eq!(Tube::Reinforced.critical_pressure(), 12.0);
        assert_eq!(Tube::Advanced.danger_pressure(), 20.0, "TIER_TWO");
        assert_eq!(Tube::Advanced.critical_pressure(), 25.0);
        assert_eq!(
            Tube::of("pneumaticcraft:advanced_pressure_tube"),
            Some(Tube::Advanced)
        );
        assert_eq!(Tube::of("pneumaticcraft:flux_compressor"), None);
        assert_eq!(Tube::of("minecraft:stone"), None);
        assert_eq!(
            Tube::Reinforced.id(),
            "pneumaticcraft:reinforced_pressure_tube"
        );
    }

    #[test]
    fn the_harness_setter_stops_at_the_tubes_critical_pressure() {
        assert_eq!(Tube::Pressure.clamped_air(999_999), 7_000);
        assert_eq!(Tube::Reinforced.clamped_air(999_999), 12_000);
        assert_eq!(Tube::Advanced.clamped_air(999_999), 100_000);
        assert_eq!(Tube::Pressure.clamped_air(-5), 0);
        assert_eq!(Tube::Pressure.clamped_air(1_600), 1_600);
    }

    #[test]
    fn sides_are_a_typed_bitmask_in_minecrafts_order() {
        let mut sides = Sides::NONE;
        assert!(sides.is_empty());
        assert_eq!(sides.the_only(), None);
        sides.insert(Side::North);
        assert!(sides.has(Side::North));
        assert!(!sides.has(Side::South));
        assert_eq!(sides.count(), 1);
        assert_eq!(sides.the_only(), Some(Side::North));
        sides.insert(Side::South);
        assert_eq!(sides.the_only(), None, "two sides are not 'the one'");
        assert_eq!(sides.count(), 2);
        assert_eq!(
            Sides::of(Side::ALL).iter().collect::<Vec<_>>(),
            Side::ALL.to_vec()
        );
        assert_eq!(Sides::all().count(), 6);
        assert_eq!(Side::Up.opposite(), Side::Down);
        assert_eq!(Side::West.name(), "west");
    }

    #[test]
    fn one_lower_neighbour_agrees_with_slice_ones_pair_rule() {
        for (mine, theirs) in [
            ((3_000, 1_000), (0, 1_000)),
            ((1_600, 10_000), (0, 10_000)),
            ((100, 10_000), (100, 10_000)),
            ((0, 10_000), (155_000, 10_000)),
            ((7_000, 1_000), (0, 4_000)),
        ] {
            let (from, to) = disperse(mine, theirs);
            let (left, moved) = disperse_to_lower(
                Handler::new(mine.0, mine.1),
                &[Handler::new(theirs.0, theirs.1)],
            );
            assert_eq!(left, from, "{mine:?} -> {theirs:?} keeps");
            assert_eq!(moved, vec![to - theirs.0], "{mine:?} -> {theirs:?} sends");
        }
    }

    #[test]
    fn three_handlers_share_by_volume() {
        // 3,000 mL in a 1,000 mL tube with two empty 1,000 mL neighbours: 1,000 each.
        let (left, moved) = disperse_to_lower(
            Handler::new(3_000, 1_000),
            &[Handler::new(0, 1_000), Handler::new(0, 1_000)],
        );
        assert_eq!(moved, vec![1_000, 1_000]);
        assert_eq!(left, 1_000);
        assert_eq!(moved.iter().sum::<i32>() + left, 3_000, "air is conserved");
    }

    #[test]
    fn only_lower_pressure_neighbours_take_part() {
        // A full tube beside an empty one and a fuller one: only the empty one is in the total,
        // so it gets its share of *this* tube's air (500), not of the three-handler total (2000).
        let (left, moved) = disperse_to_lower(
            Handler::new(1_000, 1_000),
            &[Handler::new(0, 1_000), Handler::new(5_000, 1_000)],
        );
        assert_eq!(moved, vec![500, 0], "the fuller neighbour takes nothing");
        assert_eq!(left, 500);
        // Equal pressure moves nothing at all.
        let (left, moved) =
            disperse_to_lower(Handler::new(1_000, 1_000), &[Handler::new(1_000, 1_000)]);
        assert_eq!(moved, vec![0]);
        assert_eq!(left, 1_000);
        // Nothing connected: nothing moves.
        let (left, moved) = disperse_to_lower(Handler::new(1_000, 1_000), &[]);
        assert_eq!(moved, Vec::<i32>::new());
        assert_eq!(left, 1_000);
    }

    #[test]
    fn a_bigger_neighbour_takes_its_volume_share() {
        // 1 bar in a 1,000 mL tube beside an empty 4,000 mL advanced tube: 800 to it, 200 left.
        let (left, moved) =
            disperse_to_lower(Handler::new(1_000, 1_000), &[Handler::new(0, 4_000)]);
        assert_eq!(moved, vec![800]);
        assert_eq!(left, 200);
    }

    #[test]
    fn a_tube_leaks_only_with_exactly_one_connection() {
        assert!(
            tube_leaks(Sides::of([Side::North]), Sides::NONE),
            "a dead end"
        );
        assert!(!tube_leaks(Sides::NONE, Sides::NONE), "nothing can fill it");
        assert!(
            !tube_leaks(Sides::of([Side::North, Side::South]), Sides::NONE),
            "a run holds its air"
        );
        assert!(
            !tube_leaks(Sides::of([Side::North]), Sides::of([Side::South])),
            "a closed side stops the vent"
        );
        assert!(!tube_leaks(Sides::all(), Sides::NONE));
    }

    #[test]
    fn a_single_connection_shows_its_opposite_side() {
        assert_eq!(
            visual_sides(Sides::of([Side::North]), Sides::NONE),
            Sides::of([Side::North, Side::South]),
            "the end of a run still looks like a tube"
        );
        assert_eq!(
            visual_sides(Sides::of([Side::North, Side::East]), Sides::NONE),
            Sides::of([Side::North, Side::East]),
            "a corner is shown as it is"
        );
        assert_eq!(
            visual_sides(Sides::of([Side::Up]), Sides::of([Side::Down])),
            Sides::of([Side::Up]),
            "a closed side turns the visual rule off"
        );
    }

    #[test]
    fn a_dead_end_tube_empties_as_fast_as_it_fills() {
        // 16 mL a tick in from a compressor, the tube's own vent out.
        let mut air = 0;
        for _ in 0..1_000 {
            air += 16;
            air -= tube_leak(air, VOLUME_PRESSURE_TUBE);
        }
        assert!(air <= 40, "a dead-end tube reached {air} mL");
        // At its critical pressure it vents the whole 7 bar's worth a tick.
        assert_eq!(tube_leak(7_000, VOLUME_PRESSURE_TUBE), 300);
        assert_eq!(tube_leak(0, VOLUME_PRESSURE_TUBE), 0);
        assert_eq!(
            tube_leak(10, VOLUME_PRESSURE_TUBE),
            10,
            "never more than it holds"
        );
    }
}
