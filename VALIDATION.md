# PneumaticCraft slice 1 — validation evidence

> **Historical (0.1.0, the retired `.hnmod` container).** This file is the evidence of the
> `.hnmod` port: the paths it names (`mixins/pneumaticcraft/…`, `content/`, `tools/horizon.py`)
> no longer exist, and `build-slot.sh`'s staging steps speak the retired tool. The 0.2.0 SDK
> package's ladder is in `notes.md` → "Migration to the SDK format". Kept because the behaviour
> it proves is unchanged: `src/runtime.rs` is the same file, byte for byte.

Every command below was run in this session; the quoted output is the actual result. Compile
steps went through `ports/tools/build-slot.sh`/`remote-slot.sh` (blocking leases; the remote box
takes the compile legs), and the game leg is `build-slot.sh e2e` (rebuild + e2e in one lease).

## 1. Jar, licence, gate

```
$ sha1sum pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar
bced402c1224d948554b1e8fcca2aa002c00864f
$ unzip -p … META-INF/neoforge.mods.toml | grep license
license="GNU GPLv3"
$ python3 ports/tools/precheck-mod.py pneumaticcraft
pneumaticcraft  (PneumaticCraft: Repressurized)
  class=content tier=deep kind=mech role=core prio=P0 status=pending
  licence=GPL-3.0-only rank=1 (copyleft) worldgen=False
  verdict: DISPATCHABLE
```

## 2. Pure runtime (TDD, `mixins/pneumaticcraft/runtime.rs`)

```
$ cargo test --offline            # in the port, through a slot
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The suite pins the upstream formulas and the values this slice exposes:

- `pressure(2_500, 10_000) == 0.25`, `leak(1_600, 10_000) == 26`, `leak(155_000, 10_000) == 640`
- `heat_efficiency(300.0) == 100`, `(326.0) == 99`, `(400.0) == 75`, `(401.0) == 74`, `(625.0) == 0`
- `Heat::tick` loses `1/101` of the lead to 300 K; `add_heat` divides by capacity and clamps 0..2273
- the compressor: exactly 16 mL and 40 FE a tick at ambient, 6.72 mL/tick at 42 % efficiency with
  the whole-millibucket carry (6, 7, 7, 6 over four ticks), nothing without a whole tick's FE
- the dynamo: idle at exactly 15.0 bar, 100 mL and 40 FE at 15.01 bar, **75 % / 30 FE a tick**
  once warm, idle when its buffer has no room
- dispersion equalises by volume, never flows uphill, and conserves the pair's air and FE
- a lone compressor never accumulates air (≤ 16 mL after 1,000 ticks)

**Failing-first / failure proof.** Two expectations were wrong when first run (a compressor's
"constant" 6.56 mL/tick ignored that it cools between 5-tick refreshes; one float equality needed a
tolerance) — both failed, were corrected against the model, then passed. The suite's ability to
fail was then proved by mutation: `FLUX_AIR_PER_100_FE = 40` → `100` gave
`test result: FAILED. 13 passed; 3 failed` (`the_compressor_makes_16ml_for_40fe_at_full_efficiency`,
`the_compressor_keeps_whole_millibuckets_across_ticks`, `a_lone_compressor_never_accumulates_air`),
and the constant was restored.

## 3. Format and lint

```
$ cargo fmt --check ; cargo clippy --all-targets --offline -- -D warnings
fmt=0
clippy=0
```

## 4. The bundle inside the game (pin `1d63e0a`)

```
$ python3 tools/horizon.py check
  "cargo_exit": 0        # target/horizon-check.log: 0 warnings
$ python3 tools/horizon.py test
  "cargo_exit": 0        # target/horizon-test.log: 16 passed, 0 warnings
```

The first in-game run exposed the plan's real API shape: the missing embedded content
(`native_data!` could not find it until the content was built), and a content declaration error
the game reported — `content mods not loaded: pneumaticcraft:flux_compressor: sounds minecraft:metal
is not a block` — fixed by naming a block (`minecraft:iron_block`) as the sound source. Then the
machine's rates reset every tick because the block entity is rebuilt from its compound; the rates
and the millibucket carry are now saved (`air_rate`, `fe_rate`, `air_buffer`), which took the pair
from 3.7 mL/tick to the 16 mL/tick upstream defines.

## 5. Game leg: `tools/e2e.sh`

`ports/_shared/evidence/pneumaticcraft/e2e-embedded.log` (no `--dev-mod`: the content comes from
the bundle) — the transcript's own lines:

```
== launch (The End arena, native bundle) ==
== control: the same game with no embedded content file ==
  without the embedded file: no item, the block stayed minecraft:air
  embedded content: 131324 B at …/.horizon-cache/pneumaticcraft-embedded-content.hmod,
    identical to content/pneumaticcraft.hmod
== A: the compressor makes air from FE and the dynamo shares it ==
  136 ticks: air 392 -> 2568 mL (+2176), FE 99360 -> 93920 (-5440)
== B: the dynamo idles at 15.0 bar and runs above it ==
  at 15.000 bar: air 150000 -> 150000 mL, FE 0 after 58 ticks
  at 17.840 bar: air 178400 -> 172600 mL (-5800), FE 640 -> 2878 (+2238) in 58 ticks
== C: the dynamo pushes FE into the block its coil faces ==
  sink FE 2924 -> 6148 while the dynamo kept 0 (t=727)
== C2: with no block on the coil face the dynamo keeps the FE ==
  no sink: the dynamo holds 4546 FE (t=939)
== D: a lone compressor leaks what it makes ==
  lone compressor after 118 ticks: 16 mL (the connected pair had 2568)
== E: a block that is not a slice-1 machine ==
  [302, 101, 12] is not a mod block entity.
PASS: 40 FE a tick becomes 16 mL a tick and is shared between two connected handlers; …
EXIT:0
```

`ports/_shared/evidence/pneumaticcraft/e2e-selftest.log` (`SELFTEST=1`, which flips B's
15-bar assertion so it must fail):

```
FAIL: selftest: no air was consumed at 15.0 bar (as expected)
EXIT:1
```

```
$ python3 ports/tools/check-e2e-logs.py pneumaticcraft --strict
ok       pneumaticcraft         e2e-embedded.log           PASS + EXIT:0
ok       pneumaticcraft         e2e-selftest.log           FAIL + EXIT:1
logs=2  ok=2  missing-exit=0  bad-exit=0
```

The control/differential legs, in one line each: **connected** pair +2,176 mL / −5,440 FE over
136 ticks versus **a lone compressor 16 mL** (the leak); **15.000 bar** consumes nothing versus
**17.840 bar** 100 mL/tick and FE made; a coil face **with a sink** moves 3,224 FE out versus
**no block** keeping 4,546 FE in; and the same binary **without** the embedded file knows neither
id (item and block both absent).

## 6. Packages (delivered in the port)

```
content/pneumaticcraft.hmod  131,324 B  sha256 0cae9fda259bc50b9a7e167636a38103d129b5ed4ead61e09a9b0ca2daf9f205  25 entries
pneumaticcraft.hnmod         142,531 B  sha256 d786b6a8c7004fa634bd69a5f3989ed98ee0bf68852d50b380f2f0f303cc14de  12 entries
```

`mixins/pneumaticcraft/*.rs|toml` inside the `.hnmod` were compared byte-for-byte with the port's
files (all identical), and the embedded copy equals `content/pneumaticcraft.hmod`. The `.hnmod`
carries `mixins/pneumaticcraft/data/pneumaticcraft-content.hmod`, so one file installs both the
blocks and the code.

## 7. Publishing gate

```
$ python3 ports/tools/upstream_ready.py pneumaticcraft --copyleft
BLOCKED  pneumaticcraft  GPL-3.0-only
           - status is pending, not ported
```
The remaining item is the ledger row (the orchestrator records `ported-partial`); the port's own
checks pass, and copyleft publishing is allowed under GPL-3.0 with this source and the reviewer
note.
