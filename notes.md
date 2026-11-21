# PneumaticCraft: Repressurized — port notes (slices 1–2)

Ported from **PneumaticCraft: Repressurized 8.2.24** (`pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar`,
SHA-1 `bced402c1224d948554b1e8fcca2aa002c00864f`, SHA-256
`03fa26883002b4252ec669a277b6fecf3367284e333a2165178fd236056f0b15`, 11,885,688 B, 5,679 entries)
for Minecraft Java 1.21.1 / NeoForge. Upstream
<https://github.com/TeamPneumatic/pnc-repressurized> `1.21` @ `7bcbcbc7` (0.x line, 2026-11-15).
**GPL-3.0-only → licence rank 1 (copyleft)**: the jar's `META-INF/neoforge.mods.toml` says
`license="GNU GPLv3"`, not the source map's `LGPL-2.1-only` — **the artefact wins**. The complete
corresponding source is this directory, and the full licence texts travel with it
(`LICENSE`, 701 lines of GPLv3; `COPYING.LESSER` for upstream's API). Local until the maintainer
approves publishing (ARCHITECTURE.md §8); publishing is under the same licence with this source.
Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** (Nightly 2026-11-20), mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**.

Status: **ported-partial** — slice 1 (the flux compressor, the pneumatic dynamo, their FE buffers,
the air handler with leak and dispersion, the heat model and the harness readout) and slice 2 (the
three pressure tubes and the network between them) are implemented and verified in-game at
`0a875da` (0.2.1). The gap list below is the rest of the mod.

**Why this mod now:** `python3 ports/tools/precheck-mod.py pneumaticcraft` →
`DISPATCHABLE`, class `content` / tier `deep`, core, P0, licence GPL (rank 1) read from the jar,
no unported dependency, not worldgen. The brief is
`ports/_shared/queue/brief-pneumaticcraft.md`.

## Class + tier and the approach

- **Class:** `content`; **tier:** `deep` (`ports/ledger.csv`, mirrored in
  `ports/_shared/source-map/atm10-sources.csv`).
- **Approach** (`horizon/docs/modpacks.md`): a deep mod is ported in slices; slice 1 is the
  **machines-shaped core loop** every other slice needs — a block entity per machine, a server
  tick, a declared `Capabilities.Energy.BLOCK` buffer both ways (API 142), and the pressure maths
  in a pure, unit-tested runtime. The deep half (drones, entity AI, dimensions, renderers) is not
  in this slice, and neither is the pressure **network** (tubes) — see the boundary below.
- **Carry-over vs agent port:** the two blockstates, four block models, two item models, four
  textures (+2 `.mcmeta`), the lang file, the loot tables and the two block tags are carried from
  the jar by `tools/assemble_content.py` (decisions in [`CONTENT.md`](CONTENT.md)); the
  machines, the pressure/heat rules and the commands are the agent port.

## What the slice does (upstream's own numbers)

- **Flux compressor** (`pneumaticcraft:flux_compressor`): 100,000 FE buffer; spends
  **40 FE a tick** to make **16 mL of air a tick** at 100 % heat efficiency
  (`40 × eff/100 × fluxCompressorEfficiency/100`, config default 40 air per 100 FE); a 10,000 mL
  handler with `PressureTier.TIER_TWO` (danger 20 bar, critical 25); heat exchanger capacity 100,
  `addHeat(air/20)` a tick, ambient 300 K, `(T − 300)/101` lost a tick — so it settles at
  ~300.8 K and stays at 100 % efficiency. Every 5 ticks it refreshes its rates. With nothing
  connected it **leaks** `(int)(pressure × 40) + 20` mL a tick, so a lone compressor never
  accumulates air (measured: 16 mL after 118 ticks).
- **Pneumatic dynamo** (`pneumaticcraft:pneumatic_dynamo`): 100,000 FE buffer; needs **more than
  15 bar** (`MIN_PRESSURE_PNEUMATIC_DYNAMO`); spends **100 mL a tick** for
  `40 × eff/100` FE (`pneumaticDynamoEfficiency` default 40 FE per 100 mL); a 10,000 mL
  tier-two handler; heat capacity 1 (the registry default), `addHeat(air/100)` a tick, so it
  heats towards 401 K and settles at **75 % efficiency ≈ 30 FE a tick** once warm. Every 20
  ticks it refreshes its rates. It pushes up to twice its rate into the block its **coil face**
  (`facing`) touches, through API 142 (measured: the sink gained 3,224 FE while the running
  dynamo kept 0; with no block there it kept 4,546 FE).
- **The pressure link.** Two air-handling machines facing each other are a valid PneumaticCraft
  connection (no tube needed): the pneumatic face of both is `facing.opposite`, and each tick the
  higher-pressure handler sends the lower-pressure one to its volume share
  (`totalAir × neighbourVolume / totalVolume`, no backflow) — upstream's `disperseAir`.
  Measured: 136 ticks, +2,176 mL total (16 a tick) split across both handlers, 5,440 FE spent
  (40 a tick), and no leak because the handler is connected.
- **Block state.** `active` is set on the machine's own cadence (`& 0x7` / `& 0xf`), as upstream
  writes `PNCBlockStateProperties.ACTIVE`.
- **Harness controls** (upstream has none; precedent `/powah info|set`,
  `/charginggadgets energy`): `/pneumaticcraft info <pos>` prints one parseable readout
  (`air`, `volume`, `bar`, `fe`, `temp`, `eff`, `active`, `t`); `/pneumaticcraft air <pos> <mL>`
  and `/pneumaticcraft energy <pos> <FE>` set a machine's state through the same clamp the model
  owns (critical pressure / capacity).

## Hooks (all stamped at the pin; no pending hook is named)

| Hook (API) | Helper | What it does |
|---|---|---|
| `content_root` (target, no hook version) | `embedded_content.rs` | writes the embedded `.hmod` and appends it to the dev list (AE2 pattern) |
| `block_entity` (15/129) | `block_entity.rs` | gives both blocks a block entity whatever their template |
| `block_entity_ticks` (15) | `block_entity_ticks.rs` | ticks both on the server |
| `block_entity_energy` (142) | `block_entity_energy.rs` | the 100,000 FE `Capabilities.Energy.BLOCK` buffer |
| `block_entity_tick` (17) | `block_entity_tick.rs` | one machine tick: air handler, machine, dispersion, FE push, `active` |
| `register_commands` (8) | `register_commands.rs` | `/pneumaticcraft info|air|energy` |
| `perform_command` (8) | `perform_command.rs` | runs them for the acting player |

Bundle manifest: `{"schema":1,"id":"pneumaticcraft","version":"0.1.0","game":"26.3","horizon":
"1d63e0a5…","mixins":"5a64af93…"}`. No `"hooks"` level beyond API 142 is used; the content half is a
content mod, not `native_blocks`.

## Content carried

Two blocks + their items, 25 files in `content/pneumaticcraft.hmod` (see [`CONTENT.md`](CONTENT.md)):
blockstates and models with `facing`/`active` (upstream's 8 and 12 variants), the `pnc_model`
textures, the whole `en_us.json` (267 KB, upstream's names and tooltips), the two loot tables in
26.3's singular-typed-condition shape, the pickaxe/iron-tool tags and a creative tab. The blocks
declare their state properties on a `minecraft:stone` template (`facing` 4-way / 6-way + `active`),
because 26.3 content declarations may add properties a template lacks. The two models' NeoForge
`neoforge:block/default` parent is re-spelled `minecraft:block/block`.

**One file installs both halves:** the built `content/pneumaticcraft.hmod` is carried at
`mixins/pneumaticcraft/data/pneumaticcraft-content.hmod` inside `pneumaticcraft.hnmod`;
`embedded_content.rs` writes it to `.horizon-cache/pneumaticcraft-embedded-content.hmod` and
appends it to the dev list. The e2e launches the game with **no `--dev-mod`**, checks the written
file is byte-identical to `content/pneumaticcraft.hmod`, and a control launch with
`.horizon-cache` made unusable shows both ids unknown — so the check is not vacuous.

## Deviations from upstream (recorded, not hidden)

1. **The air handler's saved shape is flat** — `air` (int) and `temperature` (double) on the
   block entity, not upstream's nested `AirHandler` compound and per-exchanger tag. The numbers
   are the same; a save written by upstream is not read.
2. **Transient rates are saved** (`air_rate`, `fe_rate`, `air_buffer`) because this port rebuilds
   its block entity from the compound every tick. Upstream keeps them as fields and recomputes
   them within 5/20 ticks of a load; ours survives a reload exactly.
3. **The FE buffer takes all six faces.** API 142 declares faces for a block id, not a block
   state, so the compressor's "every face but `facing`" and the dynamo's "coil face only" are not
   expressible; both buffers take all six faces. The machines' own intake/output logic is
   unchanged. (`native_energy` still answers per face, so this is only wider than upstream.)
4. **Ambient temperature is the 300 K base.** Upstream adds a biome term (25 × (biome − 0.8)) and
   a height term (0.1 × (sea level − y)); the pin has no biome-temperature helper, so the
   plains/sea-level value is used everywhere. At the arena (End, y = 101) upstream would be
   warmer by a degree or so; the efficiency stays 100 % either way.
5. **No menus.** Upstream's GUI (upgrade slots + redstone modes) is not in this slice; the
   machines run as upstream's default "ignoring redstone" mode does, and the harness commands are
   the readout.
6. **Item components are not registered** (`pneumaticcraft:air`, `:be_saved_rc`, `:upgrades`), so
   the carried loot tables drop the plain item (their `copy_components` functions are removed).

## Slice 2 — the pressure-tube network (0.2.1, 2026-11-21)

### The slice, and the gap line it comes from

`notes.md`'s own gap list, first bullet, verbatim:

> **Pressure tubes** (`pressure_tube`, `advanced_pressure_tube`, `reinforced_pressure_tube`,
> `tube_junction`) and tube modules — the network proper: six-sided connection state, tube
> volume aggregation, leaks over a network, modules, camouflage. This is the next slice and the
> block most of the mod stands on.

Slice 2 is the network half of that line: the **three tubes, their six-sided connection state,
their volumes and tiers, dispersion across the network and the dead-end leak**. The rest of the
line (wrench/closed sides, modules, camouflage, `tube_junction`) stays in the gap list, because
each is a different mechanism: the wrench changes `sidesClosed`, modules are a block-entity
inventory of their own, camouflage is a client render path, and a junction holds two independent
channels in one block entity. It fits the tier (`deep`) as a machines-shaped block family with a
block entity and a tick — no renderer, entity AI or dimension work.

### What upstream does (read, not guessed)

Source: `pnc-repressurized` `1.21` @ `7bcbcbc7` (2026-11-15), cloned to scratch for this job and
deleted after the findings were recorded; the jar's own Patchouli page agrees on the numbers.

| Upstream | Where | Port |
|---|---|---|
| every tube is its own `MachineAirHandler(tier, volume)` | `PressureTubeBlockEntity.java:80-85`, `:83` | one block entity type, volume from the block id (`Tube::volume`) |
| `pressure_tube` 1,000 mL / TIER_ONE (danger 5, critical 7); `reinforced` 1,000 mL / TIER_ONE_HALF (10, 12); `advanced` 4,000 mL / TIER_TWO (20, 25) | `PneumaticValues.java:82,90`, `PressureTier.java`, `ReinforcedPressureTubeBlockEntity`, `AdvancedPressureTubeBlockEntity` | `src/tubes.rs` constants, pinned by a test |
| connections are *discovered*: every face that reaches an air-handler capability, through `canConnectPneumatic` (a machine: `side == facing.opposite`; a tube: any side) | `PressureTubeBlockEntity.discoverConnectedNeighbors` (`:346-359`), `canConnectPneumatic` (`:322-327`) | `connected_handler` in `src/support.rs`, both directions |
| a tube with one connection also **shows** the opposite side | `calculateVisuallyConnected` (`:361-376`) | `tubes::visual_sides`, written to the six block state booleans each tick |
| dispersion: only *strictly lower-pressure* neighbours; total over self + those neighbours only; each gets `total_air × neighbour_volume / total_volume − neighbour_air`, never negative | `MachineAirHandler.disperseAir` (`:295-323`) | `tubes::disperse_to_lower` (and it agrees with slice 1's pair rule at N = 1) |
| a tube vents only with **exactly one** connected side (and no closed side, no module): `(int)(pressure × 40) + 20` mL, capped at what it holds | `PressureTubeBlockEntity.tickServer` (`:194-225`), `MachineAirHandler.handleAirLeak` (`:203-241`) | `tubes::tube_leaks` + `runtime::leak` |
| the flux compressor leaks only when it has **no** connected handler, so a machine feeding a tube stops leaking | `FluxCompressorBlockEntity.java:115` | unchanged from slice 1: its one pneumatic face now reaches a tube |

The tube's shape is upstream's `CORE_SHAPE` + `ARM_CONNECTED`: a 4-pixel core with 4-pixel arms
(`PressureTubeBlock.java:71-83`). The content declaration reproduces it with `thickness: 4.0` on
the chorus plant's `PipeBlock` template (`horizon-world`'s `pipe_shape` is the same geometry), and
the six block state booleans are upstream's `sidesVisuallyConnected` model data.

### Hooks

| Hook (API) | Where | What it does |
|---|---|---|
| `block_entity_tick` (17) | `TubePart::block_entity_tick` → `support::tube_tick` | one tube tick: connections, dispersion, leak, the six booleans |
| `level_tick_post` (1) | `Mod::level_tick_post` → `support::apply_pending` | delivers every handler write queued this tick (dispersion sends and the harness's sets) |
| `block_entity`, `block_entity_ticks` (15) | `TubePart` | the tube type's compound and server tick |
| `block_entity_type` | `Registrar::block_entity_type("pressure_tube").blocks(&[...3 blocks])` | one type over the three tubes |

`level_tick_post` is new to the port and is **load-bearing**, not a convenience — see the trap
below.

### Deviations (recorded, not hidden)

7. **The connection state is on the block state, not in model data.** The pin has no custom model
   loader, so upstream's `CONNECTION_PROPERTY`/`PressureTubeModel` cannot be ported; the six
   `down/up/north/south/east/west` booleans the chorus plant template already has carry the same
   information, and the blockstate is a multipart of the two carried Blockbench parts (the arm for
   a connected side, the face plate for a free one) rotated exactly as
   `PressureTubeGeometry.ROTATIONS` rotates them.
8. **The tube's saved shape is flat too** — `air` (int) on the block entity, like the machines';
   upstream saves a nested `AirHandler` compound. A save written by upstream is not read.
9. **Dispersion is applied after the tick pass.** Upstream mutates its neighbours' live handlers
   inside each handler's tick; the SDK hands a tick a compound it writes back, so a write to a
   neighbour during a tick is lost (measured, below). This port computes the same sends in the
   tick and delivers them in `level_tick_post`, as a delta on the neighbour's current air. The
   arithmetic is upstream's; only the moment of the write differs.
10. **The `active` property on a tube does not exist** (upstream has none), and `temp`/`eff`/`fe`
    in the harness readout are fixed at `300.0`/`100`/`0` for a tube: it has no heat exchanger and
    no FE buffer. A tube's readout ends with `links=<n>`, its **real** connection count (the
    block state cannot show it: a single connection also shows its opposite side).

### The trap this slice found (worth more than the slice)

**A write to a block entity made during a tick is overwritten by that block entity's own write-back
— including a write made by another block entity's tick, and by a command.** Slice 1 never saw it
because a pair's sends are ~16 mL a tick; a 4-handler network at 25 bar sends 70,000 mL a tick, so
the loss (about 200,000 mL in 40 ticks) emptied the network within a second. It was measured, not
theorised: a closed ring of four tubes (where no leak is possible) kept all 4,000 mL only once the
send was queued (`apply_pending`), and the harness's own `/pneumaticcraft air` was losing whole
sets for the same reason — two of three 25-bar sets vanished. Both now go through the queue, which
is why the e2e reads 24.9 / 24.8 / 24.7 / 24.6 bar after filling 25 / 25 / 25 / 25.

The corollary for the next port: **any mod that mutates another block entity — a cable network, a
hopper chain, a pusher — must do it at `level_tick_post`, as a delta on the target's current value,
or it loses the write.**

### Red → green (TDD, quoted)

`src/tubes.rs` was written with the rules as `todo!()` and their tests beside them:

```
$ cargo test --offline                     # ports/_shared/evidence/pneumaticcraft/tubes-red.log
thread 'tubes::tests::three_handlers_share_by_volume' panicked at src/tubes.rs:247:5:
not yet implemented: the network's dispersion
thread 'tubes::tests::a_tube_leaks_only_with_exactly_one_connection' panicked at src/tubes.rs:256:5:
not yet implemented: the tube leak predicate
thread 'tubes::tests::a_single_connection_shows_its_opposite_side' panicked at src/tubes.rs:262:5:
not yet implemented: the tube's displayed connections
...
test result: FAILED. 26 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
```

7 failed, and one of them was a real error in the first implementation (advanced tube
`danger_pressure` returned 25 instead of 20: `left: 25.0 right: 20.0`). A second expectation was
wrong in the test, not the code — `only_lower_pressure_neighbours_take_part` expected 1,000 to an
empty neighbour where upstream's own total gives 500; the test was corrected against
`MachineAirHandler.disperseAir`. Then `tubes-green.log`: **36 passed, 0 failed**.

Failure capability re-proved by mutation (`tubes-mutation.log`): `VOLUME_ADVANCED_PRESSURE_TUBE`
4,000 → 5,000 and `tube_leaks`'s `closed.is_empty() &&` → `||` gave
`test result: FAILED. 30 passed; 3 failed` (`the_three_tubes_have_upstreams_volumes_and_tiers`,
`the_harness_setter_stops_at_the_tubes_critical_pressure`,
`a_tube_leaks_only_with_exactly_one_connection`); both were restored.

### Verified in game (0.2.1, pin `0a875da`)

`ports/_shared/evidence/pneumaticcraft-e2e/` — `e2e-launcher.log` (PASS + `EXIT:0`),
`e2e-control.log` (the same assertions on the unmodified Nightly: `FAIL: the control game does not
know pneumaticcraft:pressure_tube (as expected)` + `EXIT:1`), `e2e-selftest.log` (`FAIL: selftest:
the ring kept all 4000 mL (as expected)` + `EXIT:1`), and `check-e2e-logs.py pneumaticcraft-e2e
--strict` → `logs=3 ok=3`.

| Phase | What the log shows |
|---|---|
| A | slice 1 unchanged: 139 ticks, air +2,216 mL, FE −5,560 (16 mL and 40 FE a tick) |
| B | a compressor → two advanced tubes → dynamo line at 25 bar: `links=2` on both tubes, tubes 99,336→97,236 and 99,016→96,868 mL, dynamo FE 3,648→9,490, compressor FE 98,320→91,440 in 172 ticks, network 24.90→24.14 bar |
| C | the six booleans: `west=true east=true north=false south=false up=false` on both tubes of the line, and a tube hung off one compressor shows `west=true east=true` (the opposite-side rule) with `north=false` |
| D | a closed 2×2 ring of basic tubes: 4,000 mL splits to 1,000/1,000/1,000/1,000 and the total stays 4,000 (the differential against E) |
| E | a dead-end tube (`links=1`) holds 16 mL after 221 ticks while the run's tube held 99,336: it vents what it is sent |
| F | tiers and clamps: 1,000 mL at 7.000 bar, 1,000 mL at 12.000 bar, 4,000 mL at 25.000 bar, and each alone (`links=0`) keeps what it was given |
| G | all three tube items are in the inventory |
| H | a block that is not a PneumaticCraft machine answers "not a …" |

Package: `pneumaticcraft.hmod` 227,900 B, SHA-256 `5daf7601…3fbd`, 61 entries, no `Cargo.lock`;
two builds with `rm -f Cargo.lock` each are `cmp`-identical; `check` exit 0; `fits --build 0a875da`
→ `0a875da  made for`, `fits --build 0b14f8a` → `0b14f8a  fits`; `verify-sdk-package.py
pneumaticcraft` → 1/1.

### Coverage (every family this package ships)

| Family | Where | State |
|---|---|---|
| flux compressor (FE→air, heat, `active`, leak) | slice 1 | `tested` — e2e A, and the slice-1 runs |
| pneumatic dynamo (air→FE, 15-bar gate, coil-face push) | slice 1 | `tested` — e2e B (its FE comes from air that crossed both tubes) |
| `pressure_tube` block + item, state, shape, loot | slice 2 | `tested` — e2e D/E/F, G |
| `reinforced_pressure_tube` | slice 2 | `tested` — e2e F (volume 1,000, 12.000 bar) |
| `advanced_pressure_tube` | slice 2 | `tested` — e2e B/F (volume 4,000, 25.000 bar) |
| six-sided connection state | slice 2 | `tested` — e2e C |
| network dispersion by volume, no backflow | slice 2 | `tested` — e2e B, D |
| dead-end leak, run conservation | slice 2 | `tested` — e2e D (ring keeps 4,000) and E (16 mL) |
| `/pneumaticcraft info|air|energy` | slice 1+2 | `tested` — every phase parses `info`; `air` in B/D/F; `energy` in A/B/E |
| tubes' recipes | — | `not ported` (need `compressed_iron_ingot`/`c_glass_blocks`, and the item chain is a later slice: an unknown **item** ingredient fails the content check) |
| wrench (`sidesClosed`), modules, camouflage, waterlogging, `tube_junction` | — | `not ported` — gaps (see the list above) |
| upgrades, menus, overpressure explosion, the other machines | — | `not ported` — gaps |

`## Upstream defects fixed`: none found in this slice. The two failures the TDD run caught were
this port's own (a wrong tier constant, a wrong test expectation), not upstream's; nothing in the
tube's code contradicts the jar's data. One thing worth knowing rather than fixing: upstream's
`getLeakDir` vents a dead-end tube from the side **opposite** its only connection, which is what
this port does too (the leak amount does not depend on the direction).

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices, rust-review

## Gaps (the rest of the mod; slice 3+)

- **Pressure tubes' remaining half** — the pneumatic wrench (`sidesClosed`), tube modules
  (pressure gauge, safety valve, regulator, gauges, thermometers…), camouflage, waterlogging, and
  `tube_junction` (one block entity with two independent crossing channels; not in slice 2).
- **Upgrades** (speed/volume/security/dispenser) and their items.
- **Menus/screens**, the pressure gauge/manometer item, the GUI redstone modes.
- **Overpressure explosion** (`doOverpressureChecks`) — not ported; nothing in the slice reaches
  20 bar without the harness, and the harness clamps at the critical 25 bar.
- **Every other machine family** in the jar: the other compressors (fuel, advanced, liquid,
  thermal, solar, electrostatic, manual, creative, vacuum pump), the heat system (heat pipes,
  heat sinks, vortex tube), processing (refinery, thermopneumatic processing plant, fluid mixer,
  assembly), storage/transport (tanks, hoppers, chests), mobility (elevator, door), drones,
  weapons, spawner machines, security, sensors, lamps, plastic/reinforced brick sets.
- **Recipes** for the two blocks (they need `compressed_iron_gear`, `printed_circuit_board`,
  `turbine_rotor`, `advanced_pressure_tube`).
- **Item/mod components**, block voxel shapes (full-cube collision and outline), waterlogging,
  burning particles and sounds.

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices, rust-port

## Verification

| Step | Command | Result |
|---|---|---|
| jar + licence | `sha1sum`/`unzip -p … neoforge.mods.toml` | SHA-1 `bced402c…`, `license="GNU GPLv3"` |
| gate | `python3 ports/tools/precheck-mod.py pneumaticcraft` | exit 0, `DISPATCHABLE`, GPL rank 1 |
| runtime tests (port) | `cargo test --offline` (slot) | **16 passed**, 0 failed |
| failure proof | `FLUX_AIR_PER_100_FE` 40→100, rerun | **3 tests failed**, then restored |
| format/lint | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | `fmt=0`, `clippy=0` |
| in-game check | `python3 tools/horizon.py check` | `cargo_exit: 0`, **0 warnings** |
| in-game tests | `python3 tools/horizon.py test` | **16 passed**, `cargo_exit: 0`, **0 warnings** |
| content package | `cargo-horizon-mod build` + `check` | `131,324 B`, 25 entries, `no code: content only` |
| e2e | `build-slot.sh e2e … -- tools/e2e.sh` | `PASS`, `EXIT:0` (`evidence/e2e-embedded.log`) |
| e2e selftest | `SELFTEST=1 …` | `FAIL: selftest: no air was consumed at 15.0 bar`, `EXIT:1` (`evidence/e2e-selftest.log`) |
| log checker | `python3 ports/tools/check-e2e-logs.py pneumaticcraft --strict` | `logs=2 ok=2 missing-exit=0 bad-exit=0` |
| packages | `sha256sum`, `unzip -l` | `.hmod` `0cae9fda…` / `.hnmod` `d786b6a8…` (12 entries, sources byte-identical) |
| publishing gate | `python3 ports/tools/upstream_ready.py pneumaticcraft --copyleft` | blocked only on the ledger row still reading `pending` |

## Review repairs (PR #587, 2026-11-11)

Review: `ports/_shared/horizon-mods/reviews/pr-587-pneumaticcraft.md` (CHANGES_REQUESTED, 2 items).

| Reviewer's item | This job |
|---|---|
| 1. artifact not staged | **orchestrator** (no write access) |
| 2. relative link `(TeamPneumatic/pnc-repressurized)` | already absolute; re-checked |
| (prose) the description's bullets run together mid-sentence | **rewritten**: the FE buffer, the air rate, the 10,000 mL handler, the heat exchanger numbers, the leak measurement, and the tube/upgrade/menu/explosion gaps |

The reviewer could not read GPL source; the port carries the GPL-3.0-only text and the
complete corresponding source, so a permitted reviewer has what the rule needs.

Artifact: `pneumaticcraft.hnmod` **173,869 B** `543b9a8e4ea48858b2d5e21790a216d0fd7abd2012692e731d17c5eca779d1a4`
(content `pneumaticcraft.hmod`).

## Migration to the SDK format (0.2.0, 2026-11-18)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, PR #17992) retired `.hnmod`: the registry
closed every `.hnmod` submission on 2026-11-18 with one reason, and an `.hnmod` no longer loads.
This port's former submission, **PR #802** (`pneumaticcraft` 0.1.0), was closed by that purge.
**0.2.0** is the same slice republished as one flat SDK `.hmod`; the changelog text is at the end
of this section.

Pins: SDK nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (2026-11-18, `package_format` 6,
`content_api` "0.2"), mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, for 26.3. The SDK is
cached in-tree at `../_shared/sdk/0b14f8a/` and was copied into `.horizon/sdk/` (gitignored) so
the build stays offline. The reference recipe was `ports/imfast/notes.md` → "Migration to the SDK
format", with `ports/enderstorage/notes.md` (block entities, saved compounds) and
`ports/charginggadgets/notes.md` (a block entity Part with an energy declaration) read as the
closest shapes.

**Licence, re-read for this job.** The brief said "none recorded"; `precheck-mod.py` now reports
`licence=GPL rank=1 (copyleft)` from
`pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar` — the jar's `META-INF/neoforge.mods.toml`
carries `license="GNU GPLv3"` — and flags that the ledger row still says `GPL-3.0-only` from
before the jar read (the JAR value wins; the ledger is the orchestrator's to fix, not this job's).
The port stays **local-only until the maintainer approves publishing** (rank 1, ARCHITECTURE §8);
nothing in this migration changes that. `LICENSE` (GPLv3) and `COPYING.LESSER` (upstream's API
licence) now travel **inside** the package, with `LICENSE-NOTICE.md` naming the jar and its hashes.

### What moved, and where the retired tree is

Everything the retired container owned was **moved** (never copied) into
`../_shared/evidence/pneumaticcraft/retired-hnmod-0.1.0/` **before** the first
`cargo horizon-mod build`, because the project root may hold package paths only:

| 0.1.0 path | Now | Evidence |
|---|---|---|
| `mixins/pneumaticcraft/runtime.rs` | `src/runtime.rs`, copied with `cp`, `cmp`-checked | `retired-hnmod-0.1.0/mixins/pneumaticcraft/runtime.rs` |
| `mixins/pneumaticcraft/support.rs` + 7 hook helpers | `src/support.rs` (glue) + `src/lib.rs` (the one `Hooks`) | `retired-hnmod-0.1.0/mixins/pneumaticcraft/` |
| `mixins/pneumaticcraft/horizon-mixin.toml` (7 `[[injection]]`, 2 `[[module]]`) | no plan: a `Registrar` declaration and typed `Hooks`/Part methods | `retired-hnmod-0.1.0/mixins/` |
| `mixins/pneumaticcraft/data/pneumaticcraft-content.hmod` + `embedded_content.rs` (AE2 pattern) | gone: one `.hmod` carries `assets/` and `data/` itself | `retired-hnmod-0.1.0/mixins/` |
| `content/` (`horizon-mod.toml`, `assets/`, `data/`) | the port root: `assets/`, `data/`, and the root `horizon-mod.toml` | `retired-hnmod-0.1.0/content/` |
| `content/pneumaticcraft.hmod` (131,324 B content carry) | not needed | `retired-hnmod-0.1.0/content/` |
| `pneumaticcraft.hnmod` (173,869 B, embeds the content) | `pneumaticcraft.hmod` (172,117 B, native + content) | `retired-hnmod-0.1.0/pneumaticcraft.hnmod` |
| `compatibility.json`, `rust-toolchain.toml`, `Cargo.lock` | `[native]` in `horizon-mod.toml` is the pin; the launcher's toolchain is the one that builds; the lock is not kept | `retired-hnmod-0.1.0/` |
| `tools/horizon.py`, `tools/package.py`, `tools/package.sh`, `tools/notice.py`, `tools/hmod-load.sh`, `tools/probe_content.sh`, `PLAN.md` | deleted: staging, `.hnmod` packaging, the content-notice stamper, the content-only load probe and the old plan all belong to the retired format | `retired-hnmod-0.1.0/tools/`, `retired-hnmod-0.1.0/PLAN.md` |

**Kept:** `tools/e2e.sh` (updated for a launcher-built game), `tools/parse_info.py` (unchanged),
`tools/assemble_content.py` (now writes `assets/`+`data/` at the package root and no longer writes
a manifest), `notes.md`, `FINDINGS.md`, `CONTENT.md`, `VALIDATION.md` (marked historical),
`README.md` (rewritten), `LICENSE`, `COPYING.LESSER`.

### File map (0.2.0)

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest: `id = "pneumaticcraft"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]`, `[native]` pinned to `0b14f8a`/`e59da90` |
| `Cargo.toml` | one crate, `edition = "2024"`, `license = "GPL-3.0-only"`; its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`: `register`, `register_commands`, `run_mod_command` |
| `src/support.rs` | the glue: both block entity types, their FE declarations, the two Parts, the machine tick, the neighbouring air handler, the readout and the setters. 7 tests |
| `src/runtime.rs` | the pure truth, **byte-identical** (sha256 `189e13517d0c55d8de0e617eb03a3b910e52c2e04746b2dba199833261425712`), 16 tests |
| `assets/`, `data/` | the carried content, moved from `content/` (17 + 7 files, all unchanged) |
| `LICENSE`, `COPYING.LESSER`, `LICENSE-NOTICE.md` | GPLv3, upstream's API licence and the provenance notice, **inside** the package |
| `tools/e2e.sh`, `tools/parse_info.py`, `tools/assemble_content.py` | the in-game scenario, its readout parser, the jar carry |
| `pneumaticcraft.hmod` | the delivered package |

### Module → hook → declaration table

The retired plan had 2 `[[module]]` and 7 `[[injection]]`; each one has exactly one home here.
`src/runtime.rs` became an ordinary Rust module (no plan section, no support module needed for it).

| 0.1.0 | 0.2.0 | Mapping note |
|---|---|---|
| `[[module]] runtime` (injected as `crate::game::pneumaticcraft`) | `pub mod runtime;` | same file, same 16 tests, now compiled as the crate's own module |
| `[[module]] state` (`support.rs`, `pneumaticcraft_state`) | `pub mod support;` | the glue, called from `lib.rs` and the Parts |
| `embedded-content` (`content_root`, HEAD, cancellable) | **none** | one flat `.hmod` carries `assets/`+`data/`; the write-out trick has nothing left to do |
| `block-entity` (`block_entity` 15, `block_entity.rs`) | the two **types** declared in `Hooks::register` (`block_entity_type(...).blocks(...).build()`) + `BlockEntityPart::block_entity` answering an empty compound | a declared type is what gives a block its block entity |
| `block-entity-ticks` (15, `block_entity_ticks.rs`) | `BlockEntityPart::block_entity_ticks` on both Parts → `event.ticks = true` | one Part per machine, so neither has to check its own id |
| `block-entity-energy` (142, `block_entity_energy.rs`) | `Registrar::block_energy(&block, battery())` twice | **a declaration, not a hook** — keyed by block, so the old "leave a foreign block's entry alone" hazard is gone |
| `block-entity-tick` (17, `block_entity_tick.rs`) | `BlockEntityPart::block_entity_tick` → `support::tick(host, event, kind)` | `event.pos`, `event.entity.nbt` replace the `pos`/`entity` bindings; `host` replaces `game` |
| `register-commands` (8, `register_commands.rs`) | `Hooks::register_commands` → `support::declare_commands` | `NativeCommand` → `Command` |
| `perform-command` (8, `perform_command.rs`) | `Hooks::run_mod_command` → `support::run_command` | typed `CommandInvocation`: `<pos:block_pos>` → `CommandArgumentValue::BlockPos`, `<amount:integer>` → `Integer`; the split-whitespace parser is gone |

Game calls the glue used, old → new:

| 0.1.0 | 0.2.0 |
|---|---|
| `game.world.game_time` | `level.game_time(host)` (`Option<i64>`) |
| `game.native_block_property(pos, "facing" \| "active")` | `level.block_state(host, pos)` then `state.property(&host.block_states(), name)` |
| `game.native_set_block_property(pos, "active", v)` | `host.set_block_property(pos, PROP_ACTIVE, v)` |
| `game.world.block_entities.get(&(x,y,z))` + `BlockEntity::Mod` | `host.block_entity(level, pos)` → `BlockEntityRef::data(host)` |
| `game.native_with_mod_block_entity(pos, \|other\| …)` (write) | `entity.merge_data(host, patch)` (it also marks the chunk changed) |
| `game.native_energy_insert(out, side, amount, false)` | `level.capability(host, &ENERGY, out, Some(side))` + `target.insert(host, tx, amount)` inside `host.transaction` |
| `game.world.modified.insert((cx, cz))` | `BlockEntityTickReply::Changed` (the engine saves and marks the chunk); `Unchanged` when the tick changed nothing |
| `game.native_feedback(Text::literal(…))` | `host.commands().feedback(Text::literal(…))` |
| `entity.id` (a `String` on the saved entity) | `level.block(host, pos)?.id(host)` |

### The ladder (every step inside one `build-slot.sh` lease, `CARGO_INCREMENTAL=0`, no `--target-dir`)

`build-slot.sh` still speaks the retired `tools/horizon.py`, so it is used as the **slot guard**, not
as the driver: there is no staging step and no staged tree — the SDK crate compiles in place. Each
command passed `BUILD_LOCAL=1` (the far box has no copy of this port) and its `--step` name only
picked the thread count.

| Step | Command | Result |
|---|---|---|
| `runtime.rs` first | `cp` + `cmp` + `sha256sum` | identical, `189e1351…5712` (`sdk-runtime-sha256.txt`) |
| runtime tests before any glue | `cargo test --offline` | **16 passed**, 0 failed (`sdk-test-runtime-only.log`) |
| glue red | `cargo test --offline` with the glue's bodies `todo!()` | **16 passed, 7 failed** — every `support::tests` case panicked `not yet implemented` (`sdk-test-red.log`) |
| glue green | `cargo test --offline` | **23 passed**, 0 failed (16 `runtime::`, 7 `support::`) — `sdk-test.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| format | `cargo fmt --check` (**not** `--all`: that checks the fetched SDK) | exit 0, empty diff (`sdk-fmt.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | `SDK 0b14f8a … in ./.horizon/sdk` / `Built ./target/horizon-mod/pneumaticcraft.hmod`, exit 0 (`sdk-build.log`) |
| registry check | `cargo horizon-mod check pneumaticcraft.hmod` | **EXIT 0**, output quoted below (`sdk-check.log`, `sdk-check-json.log`) |
| fits the pinned build | `cargo horizon-mod fits pneumaticcraft.hmod --build nightly` | `0b14f8a  made for`, **EXIT 0** (`sdk-fits.log`) |
| same package twice | build again, `cmp` | **172,117 B**, sha256 `d1031938…56eb` both times, `cmp: identical` (`sdk-reproducible.txt`, `sdk-build-second.log`) |
| `runtime.rs` inside the package | `unzip -p pneumaticcraft.hmod src/runtime.rs \| sha256sum` | `189e1351…5712` = the retired file, byte for byte |
| content carry re-run | `python3 tools/assemble_content.py` (twice) | all **24** carried files byte-identical to the retired copy and to each other (`sdk-content-carry.log`) |
| surface | `unzip -p pneumaticcraft.hmod uses.json` | **247** entries (`sdk-uses.json`, `sdk-uses-summary.txt`) |
| skills line | `python3 ports/tools/check-skills.py --notes ports/pneumaticcraft --strict` | `Skills line present`, **SKILLS GATE: PASS** |

`check` output, verbatim:

```
pneumaticcraft 0.2.0 (native-mod, format native-1/0b14f8acfd97fa2c750e42795249dcc981178ca4/e59da9094f0c2fdda6c627fe9eff22dec6a906f0, for 26.3)
  172117 bytes, SHA-256 d10319383b43121399194569ac1faf8e1e7800e53677725a98abfd12e74f56eb, unsigned
  native source: unsandboxed; not executed by the registry check. Compile and test against the exact pinned Horizon and Mixins revisions before review.
```

`fits` output, verbatim:

```
PneumaticCraft: Repressurized:
0b14f8a  made for
```

The archive is **34 entries**: 7 root (`horizon-mod.toml`, `Cargo.toml`, `LICENSE`,
`COPYING.LESSER`, `LICENSE-NOTICE.md`, `README.md`, `uses.json`), **17** `assets/` (blockstates,
block and item models, textures with their two `.mcmeta`, `en_us.json`), **7** `data/` (the two
block declarations, the creative tab, the two loot tables, the pickaxe and iron-tool tags) and
**3** `src/` (`lib.rs`, `runtime.rs`, `support.rs`). **No `Cargo.lock`.**

`uses.json` (247 surface entries, `sdk-uses.json` + `sdk-uses-summary.txt`): **5 hooks** in 18
entries (`block_entity`, `block_entity_ticks`, `block_entity_tick`, `register_commands`,
`run_mod_command`, with their fields), **30 methods** — including
`BlockEntityPart::block_entity*`, `DefaultCapabilitiesRegistrar::block_energy`,
`BlockEntitiesHost::block_entity`, `BlockEntityRef::data`/`merge_data`,
`HostTransaction::transaction`, `BlockStatesServerHost::set_block_property`,
`Commands::feedback`, `Levels::list_levels`, `EnergyHandler::insert` — **3 Parts**, **2 macros**
(`horizon_mod`, `horizon_part`), **2 capabilities** (`horizon:energy`, `registries`), **22 traits**,
**114 types** and **56 handle methods**. There is no `block_energy` *hook* in that list, which is
the migration's clearest evidence that the old injection became a declaration.

### Behaviour that changed with the format (deliberate, and why)

- **An idle machine no longer dirties its chunk.** The retired tick wrote its compound and marked
  the chunk changed every tick; the Part now answers `BlockEntityTickReply::Unchanged` when the
  compound and the `active` property are both unchanged. A running machine still saves.
- **The `/pneumaticcraft air` acknowledgement states what the machine stored**, not what was asked
  for: 0.1.0 answered "air set to 999999 mL" while clamping to 250,000 (the critical pressure).
  A defect of the 0.1.0 harness, fixed here; the `info` readout is unchanged byte for byte.
- **A vanilla block entity reads differently.** `/pneumaticcraft info` on a chest now says
  "`[x, y, z]` is a minecraft:chest not a PneumaticCraft machine." where 0.1.0 said "…is not a mod
  block entity." (it tested for *any* mod block entity). Both contain "not a", which is what the
  e2e greps; the new sentence is the accurate one.
- **The dynamo's FE push goes through the engine's own energy capability** on the block it faces
  (inside one transaction) instead of `native_energy_insert`, so a mod or vanilla block that
  answers `Capabilities.Energy.BLOCK` takes the FE the same way upstream's does.
- **No log line** for anything: the SDK exports no logger, so `embedded_content.rs`'s failure path
  (and the helper itself) is gone rather than rewritten.
- **`tools/e2e.sh` now defaults to a launcher-built game** (`GAME=` required, optional
  `CONTROL_GAME=` for the differential); the embedded-content legs are gone with the format.

### Traps this migration hit (the next port must know)

1. **Retired paths must be gone before the first build.** Everything the old container owned
   (`mixins/`, `content/`, the `.hnmod`, `compatibility.json`, `rust-toolchain.toml`, the old
   tools, `PLAN.md`, `Cargo.lock`) moved to evidence *first*; the root may hold package paths only.
   `content/target/` (a staging dir with the old `.hmod` inside) and a stale
   `target/pneumaticcraft-0.1.0.hnmod` were simply deleted: `target/` is disposable.
2. **A block-entity tick is not told its level.** Find it with
   `host.levels().list_levels().into_iter().find(|level| level.in_place(host))`; the event only
   carries `pos`.
3. **`level.game_time(host)` is an `Option<i64>` and can be negative.** `as u64` on a negative
   value wraps to ~1.8e19, which silently breaks the 5-/20-tick rate refresh cadence (the
   compressor would refresh its rates once and never again). Clamp with `.max(0)` first, as 0.1.0
   did on `game.world.game_time`.
4. **A block entity is out of the world for its own tick**, so `level.capability(host, &ENERGY,
   own_pos, …)` answers `None` for the machine being ticked. The buffer amount therefore lives in
   `event.entity.nbt` and the dynamo subtracts the FE it pushed from the plain int. This is why the
   port needs **no** `level_tick_post` queue: it addresses one known neighbour (which is not
   ticking, so `block_entity` + `merge_data` work) and never enumerates block entities.
5. **`ModEnergyBuffer`'s amounts are `i32`**, while the capability's are `u32`. 0.1.0's
   `ENERGY_CAPACITY` is an `i32` and is passed straight in; the capability side converts.
6. **`cargo test` and `cargo clippy` write a `Cargo.lock`**, and a packed lock changes the artifact
   (and pins versions newer than the SDK's own). `rm -f Cargo.lock` immediately before the build;
   `/Cargo.lock` is in `.gitignore`; two builds then hash identically.
7. **`cargo fmt --all` reports the fetched SDK's files.** Use `cargo fmt --check`, and never let
   `cargo fmt` touch `src/runtime.rs` (it is byte-identical: `cmp` and the packaged entry's sha256
   both prove it).
8. **`cargo horizon-mod check` with no file compiles for wasm32** (and fails). `check <file.hmod>`
   is the package check; `cargo check`/`cargo test` are the native ones.
9. **`ENERGY` needs the level handle and a transaction.** `level.capability(host, &ENERGY, pos,
   side)` answers a `Capability<dyn EnergyHandler>` whose `insert` must run inside
   `host.transaction(|host, tx| …)`; asking the host directly exposes only `energy_amount`/
   `energy_capacity` through the backend, and `energy_insert` takes a `BlockSide` the capability
   builds for you.
10. **Two Parts, not one with an id check.** Attaching `FluxPart` and `DynamoPart` to their own
    types means the tick needs no `entity.id` comparison and no machine can be ticked by the
    other's Part.
11. **The dynamo's `default_facing` is `up` while both block declarations default `facing` to
    `north`.** The property is always present, so the fallback is unreachable in practice; it is
    kept (and pinned by a test) because 0.1.0 had it, not because it fires.
12. **`side = "both"`.** A dedicated-only `side = "server"` would drop the mod from every client
    installation, including single player's integrated server.
13. **`capabilities = ["registries"]` is mandatory** even though this package registers no item or
    component: it is the schema's one value, not a claim about content.
14. **`pub mod runtime;`** — a private module turns every `pub` item in the byte-identical file into
    a `dead_code` warning, and the file must not gain an `#[allow]` to hide it.
15. **The in-game leg cannot run here.** The e2e needs a game built by `horizon-launcher`
    (`native-build … --trust-native-code`), which is not installed; `cargo horizon-mod build` only
    packs. No in-game claim is made for 0.2.0 (see the gaps).

### Gaps after the migration

1. **No 0.2.0 in-game run.** `horizon-launcher` is not installed, so there is no game carrying this
   package. The 0.1.0 runs remain the behaviour's evidence
   (`../_shared/evidence/pneumaticcraft/e2e-embedded.log`, `e2e-selftest.log`); what changed is the
   *transport* of the same calls, not the rules, which are byte-identical and tested. `tools/e2e.sh`
   is updated for `GAME=<launcher-built client>` (with optional `CONTROL_GAME=`) and keeps the
   fail-capable `SELFTEST=1`; its first real run is owed to the next session with a launcher.
2. **The energy buffer takes all six faces** on both machines (a declaration is keyed by block, not
   by block state), where upstream gives the compressor every face but its `facing` and the dynamo
   only `facing`. Unchanged from 0.1.0 and recorded above.
3. **No menus/screens**, so the GUI's redstone modes and upgrade slots are still absent; the
   machines run in upstream's default "ignoring redstone" mode.
4. Everything in the slice-1 gap list above still stands: pressure tubes and the network proper,
   upgrades, the overpressure explosion, the other machine families, recipes, item components,
   voxel shapes, waterlogging, particles and sounds.

`## Upstream defects fixed`: none found in this migration (nothing in the slice contradicts itself
or the target version). The two fixes above are the **port's own** 0.1.0 defects — the harness
acknowledgement and the every-tick chunk dirtying — recorded under the behaviour section.

### Changelog for the resubmission (supersedes #802)

> **0.2.0 — republished as an SDK `.hmod`.** Horizon's mod-API merge (launcher ≥ 0.2.70) retired
> `.hnmod` and closed **#802** (`pneumaticcraft` 0.1.0); this version supersedes it. One flat
> `pneumaticcraft.hmod` (172,117 B, 34 entries) now carries the code *and* the content — the flux
> compressor and the pneumatic dynamo with their blockstates, models, textures, lang, loot tables
> and tags — built against SDK `0b14f8a` / mixins `e59da90` for 26.3, `api = "0.2"`. What changed
> for `api = "0.2"`: the mixin plan, its seven per-hook helpers and the embedded-content injection
> are gone; the two block entity types are declared in `Hooks::register`, their 100,000 FE buffers
> are `Registrar::block_energy` declarations instead of an output-writing hook, their ticks are
> `BlockEntityPart::block_entity_tick`, and the `/pneumaticcraft info|air|energy` commands are the
> typed `register_commands`/`run_mod_command` pair. `src/runtime.rs` and its 16 tests are
> byte-identical to 0.1.0 (sha256 `189e1351…5712`, also inside the package) and the slice behaves
> exactly as before: 40 FE a tick becomes 16 mL a tick, the dynamo gates at 15 bar and spends
> 100 mL a tick, dispersion is volume-proportional, a lone handler leaks. `check` and `fits` exit 0
> and two builds hash identically (`d1031938…56eb`). One honest change: a running machine no longer
> marks its chunk changed when it did nothing, and the harness's `air` reply now states the stored
> (clamped) amount. No 0.2.0 in-game run — `horizon-launcher` is not installed here.

### Publication route (campaign ruling 2026-11-18)

The submission form is gone; publishing is `horizon-rust/horizon-mods#1147` — the artifact is
attached to a release of the author's own repository under the entry's exact `file` name, a PR adds
the `[[versions]]` record and links the file, and a **maintainer** runs `scripts/stage.sh` (a red
`check` before that is the expected `fetch.sh` failure). The orchestrator's
`python3 ports/tools/publish-ready.py pneumaticcraft` does the bookkeeping; this job owns no
submission.

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices, rust-port
