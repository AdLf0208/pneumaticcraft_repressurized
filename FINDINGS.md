# PneumaticCraft: Repressurized — slice 1 findings

Source of truth: the pack jar
`ports/_shared/licence-cache/pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar`
(SHA-1 `bced402c1224d948554b1e8fcca2aa002c00864f`, SHA-256
`03fa26883002b4252ec669a277b6fecf3367284e333a2165178fd236056f0b15`, 11,885,688 B,
5,679 entries, built 2026-10-29) and the upstream repository
<https://github.com/TeamPneumatic/pnc-repressurized> (`1.21` branch, HEAD
`00b2c5fc1f995d338e9a99c761c9d72bb3fe2b67`, 2026-11-08, release 8.2.25).
The jar is 8.2.24, one patch release behind HEAD; the classes read below are
byte-identical between 8.2.24 and 8.2.25 except the pressure-gauge fix noted in
the changelog, which this slice does not touch.

## Licence (read from the jar, not the source map)

`META-INF/neoforge.mods.toml` carries `license="GNU GPLv3"`. The source map row
says `LGPL-2.1-only`; **the artefact wins** — this is **GPL-3.0-only, licence
rank 1 (copyleft, publishable with complete source)**. The upstream tree ships
the full GPLv3 text at `LICENSE` (701 lines) and the API's LGPL at
`COPYING.LESSER` (181 lines); both are copied into this port unchanged, and the
port's own source is here in full. Publishing still needs maintainer approval
(ARCHITECTURE.md §8).

## Jar inventory (what is actually in it)

5,679 entries. Content registries:

| kind | files | kind | files |
|---|---:|---|---:|
| `data/pneumaticcraft/recipe` | 459 | `assets/pneumaticcraft/blockstates` | 171 |
| `data/pneumaticcraft/advancement` | 366 | `models` | 532 |
| `data/pneumaticcraft/loot_table` | 160 | `textures` | 574 |
| `tags` | 79 | `lang` (7 files, en_us 2,274 keys) | 7 |
| `worldgen` | 12 | `sounds` | 35 |
| `structure` | 9 | `patchouli_books` | 759 |

The 171 blockstates are the mod's machine list. Families visible in the jar:
pressure generation (`air_compressor`, `advanced_air_compressor`,
`electric_compressor`, `electrostatic_compressor`, `flux_compressor`,
`liquid_compressor`, `advanced_liquid_compressor`, `solar_compressor`,
`thermal_compressor`, `creative_compressor`, `manual_compressor`,
`vacuum_pump`), air transport (`pressure_tube`, `advanced_pressure_tube`,
`reinforced_pressure_tube`, `tube_junction`), heat (`heat_pipe`, `heat_sink`,
`thermal_lagging`, `vortex_tube`, `compressed_iron_block`), processing
(`refinery`, `refinery_output`, `thermopneumatic_processing_plant`,
`fluid_mixer`, `assembly_*`), storage (`small_tank` … `huge_tank`,
`omnidirectional_hopper`, `liquid_hopper`, `smart_chest`, `reinforced_chest`),
mobility (`elevator_base`, `elevator_frame`, `elevator_caller`,
`pneumatic_door`), drones (`drone_interface`, `drone_redstone_emitter`,
`programmable_controller`, `programmer`), weapons/utility (`air_cannon`,
`sentry_turret`, `vacuum_trap`, `charging_station`, `aerial_interface`,
`security_station`, `universal_sensor`, `uv_light_box`, `spawner_extractor`,
`pressurized_spawner`, `tag_workbench`, …). Decoration/energy: `pneumatic_dynamo`,
`pneumatic_generator`, `kerosene_lamp`, plastic/reinforced brick sets and 32
wall-lamp variants.

## Why slice 1 is the FE ↔ air conversion pair

PneumaticCraft is compressed air, and **every air machine is inert without a
pressure network**: `MachineAirHandler.tick` vents a block whose connectable
face reaches no other air handler (`handleAirLeak`: `(int)(pressure * 40) + 20`
mL a tick, `PneumaticValues.AIR_LEAK_FACTOR = 40`). A lone compressor therefore
never accumulates air — measured in the model and the e2e control leg. Porting
the tube network first (six-sided connection state, volume aggregation,
dispersion, modules, camouflage: `PressureTubeBlock` 446 lines +
`PressureTubeBlockEntity` 441) is the biggest piece of the mod, not the
smallest.

The smallest honest loop is the two machines that bracket the air system and
are **complete against each other**: `flux_compressor` turns FE into air and
`pneumatic_dynamo` turns air back into FE. Two air-capable block entities face
to face are a valid PneumaticCraft pressure connection (machines disperse
directly; tubes are one way to connect, not the only one), so this slice is
end-to-end: FE in → air → FE out, with only the mod's own numbers, and every
step observable without a GUI. It also exercises the pin's two machine
interfaces this campaign cares about: `Capabilities.Energy.BLOCK` (API 142)
both ways and block-to-block energy movement (`native_move_energy`).

## Upstream behaviour extracted (files read)

- `common/block/entity/compressor/FluxCompressorBlockEntity.java` — `BASE_FE_PRODUCTION = 40`,
  energy buffer 100,000, `PressureTier.TIER_TWO`, `PneumaticValues.VOLUME_FLUX_COMPRESSOR = 10000`,
  heat exchanger thermal capacity 100. Every 5 ticks: `airPerTick = 40 * speedUsage * heatEff/100 * fluxCompressorEfficiency/100`,
  `rfPerTick = 40 * speedUsage`. While `energy >= rfPerTick`: `airBuffer += airPerTick`, whole
  millibuckets enter the air handler, `heatExchanger.addHeat(air/20)`, `energy -= rfPerTick`.
  From all faces but `facing`: FE. Pneumatic face: `facing.opposite`. Leaks from that face when
  nothing is connected.
- `common/block/entity/compressor/PneumaticDynamoBlockEntity.java` — energy buffer 100,000,
  `TIER_TWO`, `VOLUME_PNEUMATIC_DYNAMO = 10000`, min working pressure 15 bar
  (`PneumaticValues.MIN_PRESSURE_PNEUMATIC_DYNAMO = 15`). Every 20 ticks:
  `airPerTick = (int)(40 * speedUsage * 100 / max(1, pneumaticDynamoEfficiency))`,
  `rfPerTick = (int)(40 * speedUsage * heatEff / 100)`. While pressure > 15 and the buffer has
  room for a whole `rfPerTick`: `air -= airPerTick`, `heatExchanger.addHeat(airPerTick/100)`,
  `energy += rfPerTick`; then pushes up to `rfPerTick*2` FE into the block at `facing`.
  FE faces: `facing` only. Pneumatic face: `facing.opposite`. No leak call of its own.
- `common/config/CommonConfig.java` — defaults `flux_compressor_efficiency = 40`
  (“air produced by using 100 FE”), `pneumatic_dynamo_efficiency = 40`
  (“FE produced by using 100 mL”), `air_thermal_resistance = 100`,
  `ambient_temperature_biome_modifier = 25`, `ambient_temperature_height_modifier = 0.1`.
- `common/heat/HeatUtil.java` — `getEfficiency(t) = clamp((625 - t) / 3, 0, 100)` (integer
  division on the truncated temperature); `HeatExchangerLogicTicking` — `addHeat(a)`:
  `t += a / capacity` clamped 0..2273; each tick loses heat to ambient through
  `exchange` with `totalResistance = 1 + 100` (machine thermal resistance 1, air 100), ambient
  `300 K` at plains sea level; `PneumaticRegistry` default capacity 1 where a machine does not set
  one (the dynamo).
- `common/capabilities/MachineAirHandler.java` — `pressure = air / volume` (f32); `disperseAir`
  each handler tick sends air to *strictly lower-pressure* connected handlers only: for each,
  `target = (totalAir * neighbourVolume) / totalVolume`, `toDisperse = max(0, target - neighbourAir)`,
  no backflow, unlimited per tick for machines; `handleAirLeak` vents
  `min(air, (int)(pressure * 40) + 20)`.
- `common/block/FluxCompressorBlock.java`, `PneumaticDynamoBlock.java`,
  `registry/ModBlocks.java` — `strength(3.0, 10.0)`, metal sounds, map colour metal, `active`
  (boolean) plus `facing` (4-way for the compressor, 6-way for the dynamo; the dynamo's coil is
  placed against the clicked face); both are in `minecraft:mineable/pickaxe` and
  `minecraft:needs_iron_tool`.
- `common/block/entity/PneumaticEnergyStorage.java` — plain `EnergyStorage(capacity)`: both
  insert and extract are allowed; the *faces* decide which side reaches it.

## Slice boundary (what slice 1 is, and what it deliberately is not)

**In:** the two blocks and their items, carried blockstates/models/textures/lang/loot/tags;
the two block entities and their server ticks; the FE buffers (API 142); the internal air
(volume, pressure, leak, direct machine-to-machine dispersion); the heat model with the
upstream efficiency formula; the dynamo's FE push (`native_move_energy` semantics via API 142);
the `active` block state; a `/pneumaticcraft info|air|energy` harness readout. Content is
embedded in the `.hnmod` (AE2 pattern).

**Out (recorded as gaps, not faked):**

- **Pressure tubes and any network beyond two adjacent machines** — the next slice.
- **Upgrades** (speed/volume/security/dispenser): no upgrade items exist yet; the slice keeps
  the un-upgraded multipliers (1.0).
- **Menus/GUIs** — upstream has a GUI for redstone mode and upgrade slots; the slice has no
  screen, so the redstone modes are not exposed. The machines run unconditionally (upstream's
  default `RedstoneController` mode is "ignoring redstone").
- **Overpressure explosion** (`doOverpressureChecks`): not ported; recorded in `notes.md`.
  Nothing in the slice reaches 20 bar without the harness.
- **Loot `copy_components`** for `pneumaticcraft:air`/`upgrades` — the components are not
  registered, so the carried loot drops the plain item.
- **Recipes** — upstream's recipes need `compressed_iron_gear`, `printed_circuit_board`,
  `turbine_rotor`, `advanced_pressure_tube`; the blocks exist but cannot be crafted until the
  item chain is ported. Recorded, not invented.
- **Heat neighbours** (heat pipes, heat sinks, lava) and biome/height ambient modifiers: the
  slice uses the 300 K base ambient, which is the plains/sea-level value upstream computes.
- **Item components** (`pneumaticcraft:air` on dropped blocks) and the pressure gauge/manometer.
