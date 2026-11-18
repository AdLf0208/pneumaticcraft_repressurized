# PneumaticCraft: Repressurized — a Horizon port

**PneumaticCraft: Repressurized** is a Minecraft mod. A port of PneumaticCraft to MC1.12+. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [TeamPneumatic/pnc-repressurized](https://github.com/TeamPneumatic/pnc-repressurized) — licence **GPL-3.0-only**.

| | |
|---|---|
| Upstream mod | PneumaticCraft: Repressurized ([source](https://github.com/TeamPneumatic/pnc-repressurized)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `pneumaticcraft.hmod` (install it with the Horizon launcher) |
| Licence | **GPL-3.0-only** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- **Flux compressor** (`pneumaticcraft:flux_compressor`): 100,000 FE buffer; spends
- **Pneumatic dynamo** (`pneumaticcraft:pneumatic_dynamo`): 100,000 FE buffer; needs **more than
- **The pressure link.** Two air-handling machines facing each other are a valid PneumaticCraft
- **Block state.** `active` is set on the machine's own cadence (`& 0x7` / `& 0xf`), as upstream
- **Harness controls** (upstream has none; precedent `/powah info|set`,
- **An idle machine no longer dirties its chunk.** The retired tick wrote its compound and marked
- **The `/pneumaticcraft air` acknowledgement states what the machine stored**, not what was asked
- **A vanilla block entity reads differently.** `/pneumaticcraft info` on a chest now says

## Not yet ported

- **Pressure tubes** (`pressure_tube`, `advanced_pressure_tube`, `reinforced_pressure_tube`,
- **Upgrades** (speed/volume/security/dispenser) and their items.
- **Menus/screens**, the pressure gauge/manometer item, the GUI redstone modes.
- **Overpressure explosion** (`doOverpressureChecks`) — not ported; nothing in the slice reaches
- **Every other machine family** in the jar: the other compressors (fuel, advanced, liquid,
- **Recipes** for the two blocks (they need `compressed_iron_gear`, `printed_circuit_board`,
- **Item/mod components**, block voxel shapes (full-cube collision and outline), waterlogging,
- **No 0.2.0 in-game run.** `horizon-launcher` is not installed, so there is no game carrying this

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `pneumaticcraft.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

PneumaticCraft: Repressurized was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **GPL-3.0-only**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/TeamPneumatic/pnc-repressurized

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run pneumaticcraft` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/pneumaticcraft/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
