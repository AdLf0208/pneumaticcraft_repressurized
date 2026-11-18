# PneumaticCraft: Repressurized — Horizon port, slice 1

A clean-room Rust port of **PneumaticCraft: Repressurized 8.2.24** for Minecraft 26.3 on Horizon,
built against SDK nightly `0b14f8acfd97fa2c750e42795249dcc981178ca4` and horizon-mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0` (mod API `0.2`).

Slice 1 is the mod's FE ↔ air conversion pair:

- **Flux Compressor** — 40 FE a tick becomes 16 mL of compressed air a tick (config default
  40 air per 100 FE), heat-limited, into a 10,000 mL tier-two handler with a 100,000 FE buffer;
- **Pneumatic Dynamo** — above 15 bar, 100 mL a tick becomes FE against the heat efficiency, and
  the FE is pushed into the block its coil faces.

Two machines facing each other are a valid PneumaticCraft pressure connection: the higher-pressure
handler disperses into the lower one each tick, and a lone handler leaks what it makes. See
[`FINDINGS.md`](FINDINGS.md) for the upstream behaviour and [`notes.md`](notes.md) for the slice
boundary, the migration and the gap list (the pressure tubes are slice 2).

## Layout — one flat SDK package

```
horizon-mod.toml   the only manifest and identity: id, version 0.2.0, api 0.2, side, capabilities,
                   and [native] game/horizon/mixins
Cargo.toml         one crate; its only dependency is horizon-mod-api from .horizon/sdk/ (fetched,
                   never shipped)
src/lib.rs         the one #[mod_api::horizon_mod] impl Hooks (register, the two commands)
src/support.rs     the glue: both block entity types and their 100,000 FE buffers, the machine
                   tick, the neighbouring air handler, the /pneumaticcraft readout
src/runtime.rs     the pure pressure/FE/heat arithmetic — byte-identical to the retired .hnmod
                   port's file, with its 16 tests
assets/pneumaticcraft/   carried blockstates, models, textures, lang (upstream's own GPLv3 assets)
data/                    the two block declarations, the creative tab, the loot tables and the tags
LICENSE, COPYING.LESSER  GPLv3 and upstream's API licence, inside the package
tools/             assemble_content.py (the jar carry), e2e.sh (the in-game scenario),
                   parse_info.py (the readout parser)
pneumaticcraft.hmod      the delivered package
```

The retired `.hnmod` container, its mixin plan, the seven per-hook helpers and the embedded
content copy are kept as evidence under
`../_shared/evidence/pneumaticcraft/retired-hnmod-0.1.0/`.

## Build

```sh
rm -f Cargo.lock                       # a packed lock changes the artifact
cargo horizon-mod build                # -> target/horizon-mod/pneumaticcraft.hmod
cargo horizon-mod check pneumaticcraft.hmod
cargo horizon-mod fits pneumaticcraft.hmod --build nightly
```

`cargo test` (23 tests: 16 in `runtime`, 7 in `support`) and
`cargo clippy --all-targets -- -D warnings` are the compile gate; `cargo horizon-mod build` packs,
it does not compile. The in-game leg needs a game the launcher built with this package
(`horizon-launcher native-build ports/pneumaticcraft --instance <i> --trust-native-code`), which is
not installed in this environment — see `notes.md` for that gap.

## Licence

**GPL-3.0-only** (the jar's `neoforge.mods.toml`; the source map's LGPL is wrong). This directory
is the complete corresponding source; `LICENSE` is GPLv3 and `COPYING.LESSER` is upstream's API
licence. Local-only until the maintainer approves publishing (ARCHITECTURE.md §8); any published
build is under the same licence with this source.
