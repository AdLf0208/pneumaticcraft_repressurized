# pneumaticcraft (Horizon port)

A Horizon port of [pneumaticcraft](TeamPneumatic/pnc-repressurized) — GPL-3.0-only.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- Flux compressor** (`pneumaticcraft:flux_compressor`): 100,000 FE buffer; spends
- FE a tick** to make **16 mL of air a tick** at 100 % heat efficiency
- (`40 × eff/100 × fluxCompressorEfficiency/100`, config default 40 air per 100 FE); a 10,000 mL
- handler with `PressureTier.TIER_TWO` (danger 20 bar, critical 25); heat exchanger capacity 100,
- `addHeat(air/20)` a tick, ambient 300 K, `(T − 300)/101` lost a tick — so it settles at
- ~300.8 K and stays at 100 % efficiency. Every 5 ticks it refreshes its rates. With nothing
- connected it **leaks** `(int)(pressure × 40) + 20` mL a tick, so a lone compressor never
- accumulates air (measured: 16 mL after 118 ticks).

## Not yet ported

- Pressure tubes** (`pressure_tube`, `advanced_pressure_tube`, `reinforced_pressure_tube`,
- `tube_junction`) and tube modules — the network proper: six-sided connection state, tube
- volume aggregation, leaks over a network, modules, camouflage. This is the next slice and the
- block most of the mod stands on.
- Upgrades** (speed/volume/security/dispenser) and their items.
- Menus/screens**, the pressure gauge/manometer item, the GUI redstone modes.
- Overpressure explosion** (`doOverpressureChecks`) — not ported; nothing in the slice reaches
- bar without the harness, and the harness clamps at the critical 25 bar.

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: TeamPneumatic/pnc-repressurized. Port: AdLf0208. Licence: GPL-3.0-only — see [LICENSE](LICENSE);
the original notices are preserved.
