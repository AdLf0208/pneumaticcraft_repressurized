// Pure PneumaticCraft slice-1 arithmetic (standalone crate lib, injected as
// `crate::game::pneumaticcraft`). No game access lives here: the two machines'
// FE/air conversion, the air handler's pressure and leak, the heat exchanger,
// and the dispersion between two connected machines.
//
// Upstream (PneumaticCraft: Repressurized 8.2.24, read from the jar's classes
// and the `1.21` branch of TeamPneumatic/pnc-repressurized; see FINDINGS.md):
// the flux compressor spends 40 FE a tick to make `40 * eff/100 * config/100`
// millibuckets (16 at 100 % heat efficiency and the default config), adding
// `air/20` heat to a 100-capacity exchanger; the pneumatic dynamo needs more
// than 15 bar, spends `40 * 100 / config` mL a tick (100) for
// `40 * eff/100` FE, adding `air/100` heat to a capacity-1 exchanger. Both use
// `PressureTier.TIER_TWO` (danger 20 bar, critical 25) over a 10,000 mL volume.
// Heat: `efficiency = clamp((625 - T) / 3, 0, 100)` in kelvin (integer
// division, on the truncated temperature); a machine loses
// `(T - 300) / 101` a tick to the air (its own resistance 1 plus air's 100).
// Air: `pressure = air / volume`; a block with no connected handler vents
// `min(air, (int)(pressure * 40) + 20)`; two connected machines equalise by
// volume, and only the higher-pressure one sends (`disperseAir`).

/// `PneumaticValues.VOLUME_FLUX_COMPRESSOR`.
pub const VOLUME_FLUX_COMPRESSOR: i32 = 10_000;
/// `PneumaticValues.VOLUME_PNEUMATIC_DYNAMO`.
pub const VOLUME_PNEUMATIC_DYNAMO: i32 = 10_000;
/// `new PneumaticEnergyStorage(100000)` in both machines.
pub const ENERGY_CAPACITY: i32 = 100_000;
/// `FluxCompressorBlockEntity.BASE_FE_PRODUCTION` / the dynamo's base rate.
pub const BASE_FE_PRODUCTION: i32 = 40;
/// `Config.machines.fluxCompressorEfficiency` default: air per 100 FE.
pub const FLUX_AIR_PER_100_FE: i32 = 40;
/// `Config.machines.pneumaticDynamoEfficiency` default: FE per 100 mL.
pub const DYNAMO_FE_PER_100_AIR: i32 = 40;
/// The flux compressor's heat exchanger thermal capacity.
pub const COMPRESSOR_HEAT_CAPACITY: f64 = 100.0;
/// The dynamo's heat exchanger keeps the registry default capacity.
pub const DYNAMO_HEAT_CAPACITY: f64 = 1.0;
/// `HeatExchangerLogicAmbient.BASE_AMBIENT_TEMP`: 300 K, plains at sea level.
pub const AMBIENT_TEMPERATURE: f64 = 300.0;
/// `Config.heat.airThermalResistance` default.
pub const AIR_THERMAL_RESISTANCE: f64 = 100.0;
/// `HeatExchangerLogicTicking`'s default thermal resistance.
pub const DEFAULT_THERMAL_RESISTANCE: f64 = 1.0;
/// `PneumaticValues.AIR_LEAK_FACTOR`: mL leaked per bar per tick.
pub const AIR_LEAK_FACTOR: f32 = 40.0;
/// `PressureTier.TIER_TWO.getCriticalPressure()`.
pub const CRITICAL_PRESSURE: f32 = 25.0;
/// `PneumaticValues.MIN_PRESSURE_PNEUMATIC_DYNAMO`.
pub const MIN_DYNAMO_PRESSURE: f32 = 15.0;
/// `HeatExchangerLogicTicking.addHeat` clamps the temperature here.
pub const HEAT_MAX: f64 = 2273.0;

/// `BasicAirHandler.getPressure`: millibuckets over the handler's volume.
#[must_use]
pub fn pressure(air: i32, volume: i32) -> f32 {
    air as f32 / volume as f32
}

/// `HeatUtil.getEfficiency`: percent, from the temperature truncated as the
/// block entity syncs it (`getTemperatureAsInt`), integer division.
#[must_use]
pub fn heat_efficiency(temperature: f64) -> i32 {
    let truncated = temperature.trunc() as i32;
    ((625 - truncated) / 3).clamp(0, 100)
}

/// `PneumaticValues`' leak: what a handler with no connection vents in a tick
/// (`handleAirLeak`). Zero when it holds no air.
#[must_use]
pub fn leak(air: i32, volume: i32) -> i32 {
    if air <= 0 {
        return 0;
    }
    let amount = (pressure(air, volume) * AIR_LEAK_FACTOR) as i32 + 20;
    amount.min(air)
}

/// One tick of one machine, for the block state and the harness readout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Run {
    /// The machine's working branch ran this tick (upstream's `newEnabled`).
    pub ran: bool,
    /// FE the compressor spent.
    pub fe_used: i32,
    /// FE the dynamo made.
    pub fe_made: i32,
    /// Air the compressor put into its handler.
    pub air_added: i32,
    /// Air the dynamo took out of its handler.
    pub air_used: i32,
}

/// `HeatExchangerLogicTicking`: one node, exchanging with the 300 K air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heat {
    /// `setThermalCapacity`; the registry default is 1.
    pub capacity: f64,
    /// Degrees kelvin.
    pub temperature: f64,
}

impl Heat {
    #[must_use]
    pub fn with_temperature(capacity: f64, temperature: f64) -> Self {
        Self {
            capacity,
            temperature,
        }
    }

    /// `addHeat`: `T += amount / capacity`, clamped 0..2273.
    pub fn add_heat(&mut self, amount: f64) {
        if self.capacity < 0.1 {
            self.temperature = AMBIENT_TEMPERATURE;
            return;
        }
        self.temperature = (self.temperature + amount / self.capacity).clamp(0.0, HEAT_MAX);
    }

    /// One tick of exchange with the ambient air exchanger: the resistance is
    /// the machine's 1 plus air's 100.
    pub fn tick(&mut self) {
        if self.capacity < 0.1 {
            self.temperature = AMBIENT_TEMPERATURE;
            return;
        }
        let delta = (AMBIENT_TEMPERATURE - self.temperature)
            / (AIR_THERMAL_RESISTANCE + DEFAULT_THERMAL_RESISTANCE);
        self.temperature = (self.temperature + delta).clamp(0.0, HEAT_MAX);
    }

    /// `HeatUtil.getEfficiency` on this node's temperature.
    #[must_use]
    pub fn efficiency(&self) -> i32 {
        heat_efficiency(self.temperature)
    }
}

/// `MachineAirHandler.disperseAir` between two connected handlers: the
/// higher-pressure one sends the lower-pressure one to the volume-proportional
/// share, never the other way. Answers the two new air amounts.
#[must_use]
pub fn disperse(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let (from_air, from_volume) = from;
    let (to_air, to_volume) = to;
    if from_volume <= 0 || to_volume <= 0 {
        return from;
    }
    if pressure(from_air, from_volume) <= pressure(to_air, to_volume) {
        return (from_air, to_air);
    }
    let total_air = i64::from(from_air) + i64::from(to_air);
    let total_volume = i64::from(from_volume) + i64::from(to_volume);
    let target = (total_air * i64::from(to_volume) / total_volume) as i32;
    let moved = (target - to_air).max(0);
    (from_air - moved, to_air + moved)
}

/// `FluxCompressorBlockEntity`: FE in, air out, heat up.
#[derive(Debug, Clone, PartialEq)]
pub struct FluxCompressor {
    pub energy: i32,
    pub air: i32,
    pub heat: Heat,
    air_per_tick: f64,
    rf_per_tick: i32,
    air_buffer: f64,
}

impl FluxCompressor {
    #[must_use]
    pub fn new(energy: i32, air: i32, temperature: f64) -> Self {
        Self::with_rates(energy, air, temperature, 0.0, 0)
    }

    /// The same machine with the rates its last 5-tick refresh left: upstream
    /// keeps them as fields between ticks, and this port re-reads its block
    /// entity every tick, so the glue carries them.
    #[must_use]
    pub fn with_rates(
        energy: i32,
        air: i32,
        temperature: f64,
        air_per_tick: f64,
        rf_per_tick: i32,
    ) -> Self {
        Self {
            energy: energy.clamp(0, ENERGY_CAPACITY),
            air,
            heat: Heat::with_temperature(COMPRESSOR_HEAT_CAPACITY, temperature),
            air_per_tick,
            rf_per_tick,
            air_buffer: 0.0,
        }
    }

    #[must_use]
    pub fn air_per_tick(&self) -> f64 {
        self.air_per_tick
    }

    #[must_use]
    pub fn rf_per_tick(&self) -> i32 {
        self.rf_per_tick
    }

    /// The whole-millibucket carry upstream keeps in its `airBuffer` field.
    #[must_use]
    pub fn air_buffer(&self) -> f64 {
        self.air_buffer
    }

    /// Restores that carry after the glue rebuilt the machine from its block
    /// entity.
    pub fn set_air_buffer(&mut self, air_buffer: f64) {
        self.air_buffer = air_buffer.clamp(0.0, 1.0);
    }

    /// One server tick: heat exchange, the 5-tick rate refresh, then the run
    /// while the buffer holds a whole tick's FE.
    pub fn tick(&mut self, game_time: u64) -> Run {
        self.heat.tick();
        if game_time.is_multiple_of(5) {
            let efficiency = f64::from(self.heat.efficiency());
            self.air_per_tick = f64::from(BASE_FE_PRODUCTION)
                * (efficiency / 100.0)
                * (f64::from(FLUX_AIR_PER_100_FE) / 100.0);
            self.rf_per_tick = BASE_FE_PRODUCTION;
        }
        if self.energy < self.rf_per_tick {
            return Run::default();
        }
        self.air_buffer += self.air_per_tick;
        let mut added = 0;
        if self.air_buffer >= 1.0 {
            added = self.air_buffer as i32;
            self.air += added;
            self.air_buffer -= f64::from(added);
            self.heat.add_heat(f64::from(added) / 20.0);
        }
        self.energy -= self.rf_per_tick;
        Run {
            ran: true,
            fe_used: self.rf_per_tick,
            air_added: added,
            ..Run::default()
        }
    }

    /// The `/pneumaticcraft air` setter: never above the critical pressure.
    pub fn set_air(&mut self, air: i32) {
        let ceiling = (CRITICAL_PRESSURE * VOLUME_FLUX_COMPRESSOR as f32) as i32;
        self.air = air.clamp(0, ceiling);
    }
}

/// `PneumaticDynamoBlockEntity`: air in, FE out, heat up.
#[derive(Debug, Clone, PartialEq)]
pub struct PneumaticDynamo {
    pub energy: i32,
    pub air: i32,
    pub heat: Heat,
    air_per_tick: i32,
    rf_per_tick: i32,
}

impl PneumaticDynamo {
    #[must_use]
    pub fn new(energy: i32, air: i32, temperature: f64) -> Self {
        Self::with_rates(energy, air, temperature, 0, 0)
    }

    /// The same machine with the rates its last 20-tick refresh left.
    #[must_use]
    pub fn with_rates(
        energy: i32,
        air: i32,
        temperature: f64,
        air_per_tick: i32,
        rf_per_tick: i32,
    ) -> Self {
        Self {
            energy: energy.clamp(0, ENERGY_CAPACITY),
            air,
            heat: Heat::with_temperature(DYNAMO_HEAT_CAPACITY, temperature),
            air_per_tick,
            rf_per_tick,
        }
    }

    #[must_use]
    pub fn pressure(&self) -> f32 {
        pressure(self.air, VOLUME_PNEUMATIC_DYNAMO)
    }

    #[must_use]
    pub fn rf_per_tick(&self) -> i32 {
        self.rf_per_tick
    }

    #[must_use]
    pub fn air_per_tick(&self) -> i32 {
        self.air_per_tick
    }

    /// One server tick: heat exchange, the 20-tick rate refresh, then the run
    /// above the minimum working pressure while the buffer has room.
    pub fn tick(&mut self, game_time: u64) -> Run {
        self.heat.tick();
        if game_time.is_multiple_of(20) {
            let efficiency = DYNAMO_FE_PER_100_AIR.max(1);
            self.air_per_tick = BASE_FE_PRODUCTION * 100 / efficiency;
            self.rf_per_tick = BASE_FE_PRODUCTION * self.heat.efficiency() / 100;
        }
        let room = ENERGY_CAPACITY - self.energy;
        if self.pressure() <= MIN_DYNAMO_PRESSURE || room < self.rf_per_tick {
            return Run::default();
        }
        self.air -= self.air_per_tick;
        self.heat.add_heat(f64::from(self.air_per_tick) / 100.0);
        self.energy += self.rf_per_tick;
        Run {
            ran: true,
            fe_made: self.rf_per_tick,
            air_used: self.air_per_tick,
            ..Run::default()
        }
    }

    /// The `/pneumaticcraft air` setter: never above the critical pressure.
    pub fn set_air(&mut self, air: i32) {
        let ceiling = (CRITICAL_PRESSURE * VOLUME_PNEUMATIC_DYNAMO as f32) as i32;
        self.air = air.clamp(0, ceiling);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_is_air_over_volume() {
        assert_eq!(pressure(2_500, 10_000), 0.25);
        assert_eq!(pressure(155_000, 10_000), 15.5);
        assert_eq!(pressure(0, 10_000), 0.0);
    }

    #[test]
    fn heat_efficiency_follows_upstream() {
        assert_eq!(heat_efficiency(300.0), 100);
        assert_eq!(heat_efficiency(325.0), 100);
        assert_eq!(heat_efficiency(326.0), 99);
        assert_eq!(heat_efficiency(400.0), 75);
        assert_eq!(heat_efficiency(401.0), 74);
        assert_eq!(heat_efficiency(625.0), 0);
        assert_eq!(heat_efficiency(900.0), 0);
        assert_eq!(heat_efficiency(200.0), 100);
    }

    #[test]
    fn a_lone_handler_leaks_pressure_times_40_plus_20() {
        assert_eq!(leak(0, 10_000), 0);
        assert_eq!(leak(100, 10_000), 20);
        assert_eq!(leak(1_600, 10_000), 26);
        assert_eq!(leak(155_000, 10_000), 640);
        assert_eq!(leak(10, 10_000), 10);
    }

    #[test]
    fn heat_loses_one_101st_of_its_lead_to_ambient() {
        let mut heat = Heat::with_temperature(1.0, 401.0);
        heat.tick();
        assert!(
            (heat.temperature - 400.0).abs() < 1e-9,
            "{}",
            heat.temperature
        );
        let mut cold = Heat::with_temperature(100.0, 250.0);
        cold.tick();
        assert!(
            (cold.temperature - 250.495_049_5).abs() < 1e-6,
            "{}",
            cold.temperature
        );
    }

    #[test]
    fn add_heat_divides_by_capacity_and_clamps() {
        let mut heat = Heat::with_temperature(100.0, 300.0);
        heat.add_heat(2.0);
        assert!((heat.temperature - 300.02).abs() < 1e-9);
        heat.add_heat(1_000_000.0);
        assert_eq!(heat.temperature, HEAT_MAX);
        let mut cold = Heat::with_temperature(2.0, 1.0);
        cold.add_heat(-100.0);
        assert_eq!(cold.temperature, 0.0);
    }

    #[test]
    fn disperse_sends_only_downhill_by_volume() {
        assert_eq!(disperse((310_000, 10_000), (0, 10_000)), (155_000, 155_000));
        assert_eq!(disperse((16, 10_000), (0, 10_000)), (8, 8));
        // Equal pressure: nothing moves.
        assert_eq!(disperse((100, 10_000), (100, 10_000)), (100, 100));
        // The lower-pressure block never pushes back.
        assert_eq!(disperse((0, 10_000), (155_000, 10_000)), (0, 155_000));
    }

    #[test]
    fn the_compressor_makes_16ml_for_40fe_at_full_efficiency() {
        let mut compressor = FluxCompressor::new(ENERGY_CAPACITY, 0, AMBIENT_TEMPERATURE);
        for tick in 0..100 {
            let run = compressor.tick(tick);
            assert!(run.ran, "tick {tick} did not run");
            assert_eq!(run.fe_used, 40, "tick {tick}");
        }
        assert_eq!(compressor.air, 1_600, "16 mL x 100 ticks");
        assert_eq!(compressor.energy, 96_000, "40 FE x 100 ticks");
        assert_eq!(compressor.air_per_tick(), 16.0);
        assert_eq!(compressor.heat.efficiency(), 100);
        assert!(compressor.heat.temperature > 300.0 && compressor.heat.temperature < 301.0);
    }

    #[test]
    fn the_compressor_stops_without_a_whole_ticks_fe() {
        let mut compressor = FluxCompressor::new(39, 0, AMBIENT_TEMPERATURE);
        let run = compressor.tick(0);
        assert!(!run.ran);
        assert_eq!(compressor.air, 0);
        assert_eq!(compressor.energy, 39);
    }

    #[test]
    fn the_compressor_keeps_whole_millibuckets_across_ticks() {
        // 42 % heat efficiency after the first 1.98 K of cooling: 16 * 42 / 100
        // = 6.72 mL a tick, and the fraction is carried in the float buffer the
        // way upstream's is.
        let mut compressor = FluxCompressor::new(ENERGY_CAPACITY, 0, 500.0);
        let added: Vec<i32> = (0..4).map(|tick| compressor.tick(tick).air_added).collect();
        assert!(
            (compressor.air_per_tick() - 6.72).abs() < 1e-9,
            "{}",
            compressor.air_per_tick()
        );
        assert_eq!(added, vec![6, 7, 7, 6], "0.72, 0.44, 0.16, 0.88 carried");
        assert_eq!(compressor.air, 26);
    }

    #[test]
    fn the_compressor_adds_exactly_what_it_reports_and_cools() {
        let mut compressor = FluxCompressor::new(ENERGY_CAPACITY, 0, 500.0);
        let mut added = 0;
        for tick in 0..100 {
            added += compressor.tick(tick).air_added;
        }
        assert_eq!(
            compressor.air, added,
            "the handler holds what the runs added"
        );
        assert_eq!(compressor.energy, ENERGY_CAPACITY - 4_000);
        // Losing heat to the 300 K air raises the rate above the cold start's.
        assert!(
            compressor.air_per_tick() > 6.56,
            "rate stayed at {}",
            compressor.air_per_tick()
        );
        assert!(compressor.heat.temperature < 500.0);
    }

    #[test]
    fn the_dynamo_needs_more_than_15_bar() {
        // 149,999 mL over 10,000 is 14.9999 bar: below the threshold.
        let mut below = PneumaticDynamo::new(0, 149_999, AMBIENT_TEMPERATURE);
        let run = below.tick(0);
        assert!(!run.ran);
        assert_eq!(below.air, 149_999);
        assert_eq!(below.energy, 0);

        let mut above = PneumaticDynamo::new(0, 150_001, AMBIENT_TEMPERATURE);
        let run = above.tick(0);
        assert!(run.ran);
        assert_eq!(run.air_used, 100);
        assert_eq!(run.fe_made, 40);
        assert_eq!(above.air, 150_001 - 100);
        assert_eq!(above.energy, 40);
    }

    #[test]
    fn the_dynamo_settles_at_75_percent_and_30_fe_a_tick() {
        // 30 bar of air is 200,000 mL left above the gate after 800 ticks.
        let mut dynamo = PneumaticDynamo::new(0, 250_000, AMBIENT_TEMPERATURE);
        for tick in 0..600 {
            dynamo.tick(tick);
        }
        let before = dynamo.energy;
        let air_before = dynamo.air;
        for tick in 600..800 {
            dynamo.tick(tick);
        }
        assert_eq!(
            dynamo.heat.efficiency(),
            75,
            "T={}",
            dynamo.heat.temperature
        );
        assert_eq!(dynamo.rf_per_tick(), 30);
        assert_eq!(dynamo.energy - before, 6_000, "30 FE x 200 ticks");
        assert_eq!(air_before - dynamo.air, 20_000, "100 mL x 200 ticks");
    }

    #[test]
    fn the_dynamo_stops_when_its_buffer_has_no_room() {
        let mut dynamo = PneumaticDynamo::new(ENERGY_CAPACITY - 39, 200_000, AMBIENT_TEMPERATURE);
        let run = dynamo.tick(0);
        assert!(!run.ran, "40 FE does not fit in 39 FE of room");
        assert_eq!(dynamo.air, 200_000);
    }

    #[test]
    fn the_pair_converts_fe_to_air_and_back_at_upstreams_rates() {
        let mut compressor = FluxCompressor::new(ENERGY_CAPACITY, 0, AMBIENT_TEMPERATURE);
        let mut dynamo = PneumaticDynamo::new(0, 160_000, AMBIENT_TEMPERATURE);
        let mut added = 0;
        let mut used = 0;
        for tick in 0..200 {
            // The compressor disperses after its own tick, as each machine's
            // air handler does; the dynamo has no lower-pressure neighbour.
            let run = compressor.tick(tick);
            added += run.air_added;
            let run = dynamo.tick(tick);
            used += run.air_used;
            let (from, to) = disperse(
                (compressor.air, VOLUME_FLUX_COMPRESSOR),
                (dynamo.air, VOLUME_PNEUMATIC_DYNAMO),
            );
            compressor.air = from;
            dynamo.air = to;
        }
        assert!(dynamo.energy > 0, "the dynamo made no FE");
        assert_eq!(
            compressor.energy,
            ENERGY_CAPACITY - 8_000,
            "200 ticks x 40 FE"
        );
        // Every millibucket the compressor added is in a handler or was used:
        // none is created or lost, and the dynamo cannot make more FE than the
        // compressor spent.
        assert_eq!(compressor.air + dynamo.air, 160_000 + added - used);
        assert!(
            dynamo.energy < 8_000,
            "the dynamo cannot make more FE than the compressor spent"
        );
    }

    #[test]
    fn a_lone_compressor_never_accumulates_air() {
        let mut compressor = FluxCompressor::new(ENERGY_CAPACITY, 0, AMBIENT_TEMPERATURE);
        for tick in 0..1_000 {
            compressor.tick(tick);
            let lost = leak(compressor.air, VOLUME_FLUX_COMPRESSOR);
            compressor.air -= lost;
        }
        assert!(
            compressor.air <= 16,
            "a lone compressor reached {} mL",
            compressor.air
        );
    }

    #[test]
    fn the_harness_setter_stops_at_the_critical_pressure() {
        let mut compressor = FluxCompressor::new(0, 0, AMBIENT_TEMPERATURE);
        compressor.set_air(999_999);
        assert_eq!(compressor.air, 250_000);
        compressor.set_air(-5);
        assert_eq!(compressor.air, 0);
    }
}
