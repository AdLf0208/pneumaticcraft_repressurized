# PneumaticCraft: Repressurized — Horizon port, slices 1–2

A clean-room Rust port of **PneumaticCraft: Repressurized 8.2.24** for Minecraft 26.3 on Horizon,
built against SDK nightly `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` and horizon-mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0` (mod API `0.2`).

**Slice 1 — the FE ↔ air conversion pair:**

- **Flux Compressor** — 40 FE a tick becomes 16 mL of compressed air a tick (config default
  40 air per 100 FE), heat-limited, into a 10,000 mL tier-two handler with a 100,000 FE buffer;
- **Pneumatic Dynamo** — above 15 bar, 100 mL a tick becomes FE against the heat efficiency, and
  the FE is pushed into the block its coil faces.

**Slice 2 — the pressure-tube network:**

- **Pressure Tube** (1,000 mL, tier one: danger 5 bar, critical 7), **Reinforced Pressure Tube**
  (1,000 mL, tier 1.5: 10/12) and **Advanced Pressure Tube** (4,000 mL, tier two: 20/25). Each is
  its own air handler, as upstream's are.
- A tube joins every side that reaches another handler (a machine only on its pneumatic face) and
  shows the six connections on its block state; a tube with a single connection also shows the
  opposite side, so the end of a run looks like a run.
- Every tick a handler sends each **strictly lower-pressure** neighbour its share of the air by
  volume (never backwards), so a compressor → tube → tube → dynamo line carries air the whole way
  and the dynamo turns it back into FE.
- A **dead-end tube vents** `(int)(pressure × 40) + 20` mL a tick; a tube in a run holds its air,
  and a closed ring of tubes conserves it exactly.

See [`FINDINGS.md`](FINDINGS.md) for the upstream behaviour, [`CONTENT.md`](CONTENT.md) for what is
carried from the jar, and [`notes.md`](notes.md) for the slice boundary, the deviations, the
in-game evidence and the gap list.

## Layout — one flat SDK package

```
horizon-mod.toml   the only manifest and identity: id, version 0.2.1, api 0.2, side, capabilities,
                   and [native] game/horizon/mixins
Cargo.toml         one crate; its only dependency is horizon-mod-api from .horizon/sdk/ (fetched,
                   never shipped)
src/lib.rs         the one #[mod_api::horizon_mod] impl Hooks (register, the two commands, and the
                   level_tick_post that delivers queued handler writes)
src/support.rs     the glue: the three block entity types, the machines' 100,000 FE buffers, the
                   machine and tube ticks, the neighbouring air handler, the queued writes, the
                   /pneumaticcraft readout
src/runtime.rs     slice 1's pure pressure/FE/heat arithmetic — byte-identical to the retired
                   .hnmod port's file, with its 16 tests
src/tubes.rs       slice 2's pure rules: volumes and tiers, the network's dispersion, the leak
                   predicate, the displayed connections — with its 10 tests
assets/pneumaticcraft/   carried blockstates, models, textures, lang (upstream's own GPLv3 assets)
data/                    the block declarations, the creative tab, the loot tables and the tags
LICENSE, COPYING.LESSER  GPLv3 and upstream's API licence, inside the package
tools/             assemble_content.py (the jar carry), e2e.sh (the in-game scenario),
                   parse_info.py (the readout parser)
pneumaticcraft.hmod      the delivered package
```

The retired `.hnmod` container, its mixin plan, the seven per-hook helpers and the embedded
content copy are kept as evidence under
`../_shared/evidence/pneumaticcraft/retired-hnmod-0.1.0/`.

## Build and check

```sh
rm -f Cargo.lock                       # a packed lock changes the artifact
cargo horizon-mod build                # -> target/horizon-mod/pneumaticcraft.hmod
cargo horizon-mod check pneumaticcraft.hmod
cargo horizon-mod fits pneumaticcraft.hmod --build nightly
```

`cargo test` (36 tests: 16 in `runtime`, 10 in `tubes`, 10 in `support`) and
`cargo clippy --all-targets -- -D warnings` are the compile gate; `cargo horizon-mod build` packs,
it does not compile. The in-game leg builds a game with the package and drives it with `horizon-ctl`
through the campaign's wrapper:

```sh
ports/tools/launcher-game.sh run pneumaticcraft
```

## Licence

**GPL-3.0-only** (the jar's `neoforge.mods.toml`; the source map's LGPL is wrong). This directory
is the complete corresponding source; `LICENSE` is GPLv3 and `COPYING.LESSER` is upstream's API
licence. Local-only until the maintainer approves publishing (ARCHITECTURE.md §8); any published
build is under the same licence with this source.
