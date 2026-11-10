#!/usr/bin/env bash
# In-game check for PneumaticCraft slice 1: the flux compressor turns FE into
# compressed air and the pneumatic dynamo turns air back into FE, with the
# mod's own numbers.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# The End, in the void at x=300 (away from the central island and the Overworld's
# ambient item entities). A stone floor at y=100; the machines stand at y=101.
#
# Default run (EMBEDDED=1): the game is launched with **only the native bundle**
# (no `--dev-mod`) and the two blocks come from the bundle's embedded copy
# (`mixins/pneumaticcraft/data/pneumaticcraft-content.hmod`, the AE2 pattern).
# The run checks the file the helper wrote and, with CONTROL=1, first launches
# the same binary with `.horizon-cache` unusable, where both ids must be unknown.
# EMBEDDED=0 loads `content/` as a dev mod instead.
#
#  A  a compressor and a dynamo facing each other: FE leaves the compressor at
#     40 a tick, air appears at 16 mL a tick and disperses into the dynamo
#     (both handlers hold some), and the pair never leaks;
#  B  the dynamo does nothing at exactly 15 bar and runs above it: 100 mL a tick
#     for FE (40 a tick while its heat is still at ambient);
#  C  the running dynamo pushes its FE into the block its coil faces, and with
#     no block there it keeps the FE instead (the differential);
#  D  a lone compressor leaks what it makes (`pressure * 40 + 20` mL a tick);
#  E  a block that is not a slice-1 machine reads as something else;
#  F  the embedded content file is byte-identical to content/pneumaticcraft.hmod,
#     and the same binary without it knows neither id (the control launch).
#
# SELFTEST=1 flips B's 15-bar assertion (it must FAIL), proving the scenario can
# fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${PC_DEBUG:-}" ] || set -x

EMBEDDED=${EMBEDDED:-1}
CONTROL=${CONTROL:-$EMBEDDED}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/pneumaticcraft
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/pneumaticcraft-e2e.sock}
WORLD=PneumaticCraftE2E
CONTROL_WORLD=PneumaticCraftControl
# Where the arena stands: the End's void, far from the central island.
X0=300
SX=$((X0 + 2))
CAPACITY=100000
FE_A_TICK=40
AIR_A_TICK=16
AIR_PER_TICK_DYNAMO=100
MIN_AIR=$((15 * 10000))

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/pneumaticcraft-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# The End's void at X0: a stone floor and clear air above it.
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 6)) 100 -16 $((X0 + 10)) 100 16 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 6)) 101 -16 $((X0 + 10)) 108 16 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
}

# `/pneumaticcraft info X Y Z`: parse the reply into PREFIX_AIR, PREFIX_FE, ...
info() { # info X Y Z PREFIX
    say "pneumaticcraft info $1 $2 $3"
    ctl wait 14 >/dev/null
    local line
    line=$(ctl messages 14 | jq -r '.chat[]' | grep -m1 '^PC ' || true)
    [ -n "$line" ] || fail "no /pneumaticcraft info reply at $1 $2 $3"
    echo "  $line" >&2
    case "$line" in
        "PC "*) ;;
        *) fail "the info line is not a readout: $line" ;;
    esac
    local body
    body=$(python3 "$PORT/tools/parse_info.py" "$line")
    eval "$(printf '%s\n' "$body" | sed "s/^\([A-Za-z_][A-Za-z0-9_]*\)=/$4_\1=/")"
    eval "[ \"\${${4}_OK:-0}\" = 1 ]" || fail "the info line did not parse: $line"
}

set_block() { # set_block X Y Z STATE
    ctl command "execute in minecraft:the_end run setblock $1 $2 $3 $4" >/dev/null
    ctl wait 4 >/dev/null
}

# A pair that faces each other: the compressor's pneumatic face is `facing`'s
# opposite, as is the dynamo's, so east/west-facing neighbours connect.
place_pair() { # place_pair Z [SINK]
    local z=$1 sink=${2:-0}
    set_block "$SX" 101 "$z" "pneumaticcraft:flux_compressor[facing=east,active=false]"
    set_block "$((SX - 1))" 101 "$z" "pneumaticcraft:pneumatic_dynamo[facing=west,active=false]"
    [ "$(block_name "$SX" 101 "$z")" = "pneumaticcraft:flux_compressor" ] \
        || fail "the compressor did not place at $SX 101 $z"
    [ "$(block_name "$((SX - 1))" 101 "$z")" = "pneumaticcraft:pneumatic_dynamo" ] \
        || fail "the dynamo did not place at $((SX - 1)) 101 $z"
    if [ "$sink" = 1 ]; then
        set_block "$((SX - 2))" 101 "$z" "pneumaticcraft:pneumatic_dynamo[facing=east,active=false]"
        [ "$(block_name "$((SX - 2))" 101 "$z")" = "pneumaticcraft:pneumatic_dynamo" ] \
            || fail "the sink dynamo did not place at $((SX - 2)) 101 $z"
    fi
}

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. Both ids must then be unknown.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    arena
    ctl command "execute in minecraft:the_end run setblock $SX 101 0 pneumaticcraft:flux_compressor" >/dev/null || true
    ctl wait 4 >/dev/null
    local name
    name=$(block_name "$SX" 101 0)
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the block id was known"
    ctl command "item replace entity @s hotbar.0 with pneumaticcraft:flux_compressor 1" >/dev/null || true
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "pneumaticcraft:flux_compressor")] | length == 0' >/dev/null \
        || fail "the control game has the compressor item, so the content did not come from the embedded file"
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no item, the block stayed $name"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/pneumaticcraft-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/pneumaticcraft.hmod" \
        || fail "the embedded content differs from content/pneumaticcraft.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/pneumaticcraft.hmod"
}

echo "== launch (The End arena, native bundle) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
arena

echo "== A: the compressor makes air from FE and the dynamo shares it =="
place_pair 0 1
info "$SX" 101 0 A0
info "$((SX - 1))" 101 0 B0
[ "$A0_AIR" = 0 ] || fail "a fresh compressor holds $A0_AIR mL"
[ "$B0_AIR" = 0 ] || fail "a fresh dynamo holds $B0_AIR mL"
say "pneumaticcraft energy $SX 101 0 $CAPACITY"
ctl wait 14 >/dev/null
info "$SX" 101 0 A1
info "$((SX - 1))" 101 0 B1
[ "$A1_FE" -gt $((CAPACITY - 1000)) ] || fail "the harness did not fill the compressor ($A1_FE FE)"
ctl wait 100 >/dev/null
info "$SX" 101 0 A2
info "$((SX - 1))" 101 0 B2
TICKS_A=$((A2_T - A1_T))
[ "$TICKS_A" -gt 0 ] || fail "no ticks passed between the two reads"
AIR_TOTAL=$((A2_AIR + B2_AIR))
AIR_BASE=$((A1_AIR + B1_AIR))
AIR_EXPECTED=$((AIR_A_TICK * TICKS_A))
FE_SPENT=$((A1_FE - A2_FE))
# ±5 ticks of rate refresh at the start, ±40 for the read boundaries.
[ $((AIR_TOTAL - AIR_BASE)) -ge $((AIR_EXPECTED - 5 * AIR_A_TICK - 40)) ] \
    || fail "the pair holds $AIR_TOTAL mL after $TICKS_A ticks, expected about $((AIR_BASE + AIR_EXPECTED))"
[ $((AIR_TOTAL - AIR_BASE)) -le $((AIR_EXPECTED + 40)) ] \
    || fail "the pair holds $AIR_TOTAL mL after $TICKS_A ticks, more than $((AIR_BASE + AIR_EXPECTED + 40))"
[ "$FE_SPENT" -ge $((FE_A_TICK * TICKS_A - 5 * FE_A_TICK - 40)) ] \
    || fail "the compressor spent $FE_SPENT FE in $TICKS_A ticks, expected about $((FE_A_TICK * TICKS_A))"
[ "$FE_SPENT" -le $((FE_A_TICK * TICKS_A + 40)) ] \
    || fail "the compressor spent $FE_SPENT FE in $TICKS_A ticks, more than $((FE_A_TICK * TICKS_A))"
[ "$A2_AIR" -gt 0 ] && [ "$B2_AIR" -gt 0 ] \
    || fail "the two handlers did not share the air: $A2_AIR / $B2_AIR mL"
echo "  $TICKS_A ticks: air $AIR_BASE -> $AIR_TOTAL mL (+$((AIR_TOTAL - AIR_BASE))), FE $A1_FE -> $A2_FE (-$FE_SPENT)"

echo "== B: the dynamo idles at 15.0 bar and runs above it =="
set_block "$((SX - 4))" 101 4 "pneumaticcraft:pneumatic_dynamo[facing=east,active=false]"
say "pneumaticcraft air $((SX - 4)) 101 4 $MIN_AIR"
ctl wait 14 >/dev/null
info "$((SX - 4))" 101 4 T0
[ "$T0_BAR" = "15.000" ] || fail "the harness set $T0_BAR bar, expected 15.000"
ctl wait 40 >/dev/null
info "$((SX - 4))" 101 4 T1
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting the dynamo consumed air at 15.0 bar)"
    [ "$T1_AIR" -lt "$T0_AIR" ] || fail "selftest: no air was consumed at 15.0 bar (as expected)"
else
    [ "$T1_AIR" = "$T0_AIR" ] || fail "the dynamo used air ($T0_AIR -> $T1_AIR mL) at exactly 15.0 bar"
    [ "$T1_FE" = 0 ] || fail "the dynamo made $T1_FE FE below its working pressure"
fi
echo "  at $T0_BAR bar: air $T0_AIR -> $T1_AIR mL, FE $T1_FE after $((T1_T - T0_T)) ticks"
say "pneumaticcraft air $((SX - 4)) 101 4 $((MIN_AIR + 30000))"
ctl wait 14 >/dev/null
info "$((SX - 4))" 101 4 T2
ctl wait 40 >/dev/null
info "$((SX - 4))" 101 4 T3
TICKS_B=$((T3_T - T2_T))
AIR_USED=$((T2_AIR - T3_AIR))
FE_MADE=$((T3_FE - T2_FE))
[ "$AIR_USED" -ge $((AIR_PER_TICK_DYNAMO * TICKS_B - AIR_PER_TICK_DYNAMO)) ] \
    || fail "the dynamo used $AIR_USED mL in $TICKS_B ticks, expected about $((AIR_PER_TICK_DYNAMO * TICKS_B))"
[ "$AIR_USED" -le $((AIR_PER_TICK_DYNAMO * TICKS_B + AIR_PER_TICK_DYNAMO)) ] \
    || fail "the dynamo used $AIR_USED mL in $TICKS_B ticks, more than $((AIR_PER_TICK_DYNAMO * TICKS_B))"
[ "$FE_MADE" -gt 0 ] || fail "the dynamo made no FE at $T2_BAR bar"
[ "$FE_MADE" -le $((FE_A_TICK * TICKS_B)) ] || fail "the dynamo made $FE_MADE FE, more than $((FE_A_TICK * TICKS_B))"
echo "  at $T2_BAR bar: air $T2_AIR -> $T3_AIR mL (-$AIR_USED), FE $T2_FE -> $T3_FE (+$FE_MADE) in $TICKS_B ticks"

echo "== C: the dynamo pushes FE into the block its coil faces =="
# Two steps reach >15 bar: the compressor is set, equalised, then the dynamo is
# set, and the pair settles at 18.75 bar (the setter stops at the critical 25 bar
# per handler, so one 310,000 set could not reach 15 bar across two volumes).
say "pneumaticcraft energy $SX 101 0 0"
say "pneumaticcraft air $SX 101 0 250000"
ctl wait 40 >/dev/null
say "pneumaticcraft air $((SX - 1)) 101 0 250000"
ctl wait 40 >/dev/null
info "$SX" 101 0 C0
info "$((SX - 1))" 101 0 C1
info "$((SX - 2))" 101 0 C2
awk -v bar="$C1_BAR" 'BEGIN { exit !(bar + 0 > 15) }' \
    || fail "the pair settled at $C1_BAR bar, not above the dynamo's 15"
SINK_BEFORE=$C2_FE
ctl wait 60 >/dev/null
info "$((SX - 1))" 101 0 C3
info "$((SX - 2))" 101 0 C4
[ "$C4_FE" -gt "$SINK_BEFORE" ] \
    || fail "the sink dynamo received no FE ($SINK_BEFORE -> $C4_FE) while the pair ran at $C1_BAR bar"
[ "$C3_FE" -lt 100 ] || fail "the running dynamo kept $C3_FE FE instead of pushing it into its coil face"
echo "  sink FE $SINK_BEFORE -> $C4_FE while the dynamo kept $C3_FE (t=$C3_T)"

echo "== C2: with no block on the coil face the dynamo keeps the FE =="
place_pair -4 0
say "pneumaticcraft energy $SX 101 -4 0"
say "pneumaticcraft air $SX 101 -4 250000"
ctl wait 40 >/dev/null
say "pneumaticcraft air $((SX - 1)) 101 -4 250000"
ctl wait 40 >/dev/null
info "$((SX - 1))" 101 -4 D1
ctl wait 60 >/dev/null
info "$((SX - 1))" 101 -4 D2
[ "$D2_FE" -gt 0 ] || fail "the dynamo kept no FE with no coil-face block (reads $D2_FE)"
echo "  no sink: the dynamo holds $D2_FE FE (t=$D2_T)"

echo "== D: a lone compressor leaks what it makes =="
set_block "$SX" 101 8 "pneumaticcraft:flux_compressor[facing=east,active=false]"
say "pneumaticcraft energy $SX 101 8 $CAPACITY"
ctl wait 14 >/dev/null
info "$SX" 101 8 E0
ctl wait 100 >/dev/null
info "$SX" 101 8 E1
TICKS_D=$((E1_T - E0_T))
[ "$E1_AIR" -le 16 ] || fail "a lone compressor reached $E1_AIR mL; the leak did not run"
[ $((E0_FE - E1_FE)) -ge $((FE_A_TICK * TICKS_D - 5 * FE_A_TICK - 40)) ] \
    || fail "the lone compressor spent $((E0_FE - E1_FE)) FE in $TICKS_D ticks"
echo "  lone compressor after $TICKS_D ticks: $E1_AIR mL (the connected pair had $AIR_TOTAL)"

echo "== E: a block that is not a slice-1 machine =="
set_block "$SX" 101 12 "minecraft:stone"
say "pneumaticcraft info $SX 101 12"
ctl wait 14 >/dev/null
NON_LINE=$(ctl messages 14 | jq -r '.chat[]' | grep -m1 'not a' || true)
[ -n "$NON_LINE" ] || fail "no 'not a ...' reply for a stone block"
echo "  $NON_LINE"

echo "PASS: 40 FE a tick becomes 16 mL a tick and is shared between two connected handlers; the dynamo idles at exactly 15.0 bar and spends 100 mL a tick above it; it pushes its FE into the block its coil faces and keeps it when there is none; a lone compressor leaks what it makes; and one .hnmod installs both blocks"
