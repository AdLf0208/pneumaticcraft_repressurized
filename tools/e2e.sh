#!/usr/bin/env bash
# In-game check for the PneumaticCraft port: slice 1's FE/air pair and slice 2's pressure tubes.
#
# The game must be one the launcher built **with this package** — a native `.hmod` cannot load on
# an unmodified game. The campaign wrapper does both steps and the slot lease:
#
#   ports/tools/launcher-game.sh run pneumaticcraft
#
# or, with the two steps visible:
#
#   ports/tools/launcher-game.sh build pneumaticcraft
#   eval "$(ports/tools/launcher-game.sh env pneumaticcraft)"
#   ports/tools/build-slot.sh run --job W1-pneumaticcraft --port ports/pneumaticcraft --step e2e \
#       -- bash ports/pneumaticcraft/tools/e2e.sh
#
# `CONTROL_GAME` (the same Nightly without the package) runs the differential first: the same
# block and item ids must be unknown there, so the real run cannot pass vacuously.
#
# Every mod command is typed with `ctl chat "/cmd"` — the route that reaches a mod's command hook.
#
# The End, in the void at x≈300 (away from the central island and the Overworld's ambient item
# entities). A stone floor at y=100; the blocks stand at y=101.
#
#  A  slice 1 still runs: a compressor beside a dynamo makes 16 mL a tick for 40 FE a tick and
#     shares it with the dynamo;
#  B  the network carries air: a compressor, two tubes and a dynamo at 25 bar, and the dynamo runs
#     off air that has crossed both tubes while the compressor and the tubes refill it;
#  C  the six connection booleans follow the network: a tube in a run shows the two sides it joins
#     and nothing else, and a tube hung off one machine shows its opposite side too;
#  D  a closed 2x2 ring of tubes split 4,000 mL into 1,000 each and loses none of it (the
#     differential against E);
#  E  a dead-end tube vents what the compressor sends it instead of filling;
#  F  the three tubes have upstream's volumes and critical pressures: 1,000 mL at 7 bar,
#     1,000 mL at 12 bar, 4,000 mL at 25 bar, and a lone tube keeps what it is given;
#  G  all three tube items are in the game;
#  H  a block that is not a PneumaticCraft machine reads as something else.
#
# SELFTEST=1 flips D's conservation assertion (it must FAIL), proving the scenario can fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${PC_DEBUG:-}" ] || set -x

SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}

PORT=$(cd "$(dirname "$0")/.." && pwd)
CTL=${CTL:-horizon-ctl}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/pneumaticcraft-e2e.sock}
WORLD=PneumaticCraftE2E
CONTROL_WORLD=PneumaticCraftControl
# Where the arena stands: the End's void, far from the central island.
X0=300
CAPACITY=100000
FE_A_TICK=40
AIR_A_TICK=16
BAR_TUBE=7000
BAR_REINFORCED=12000
BAR_ADVANCED=100000
# 25 bar across a compressor (10,000 mL), two advanced tubes (4,000 each) and a dynamo
# (10,000): 250,000 + 100,000 + 100,000 + 250,000 mL over 28,000 mL = 25 bar.
BAR_25_FEEDER=250000
BAR_25_TUBE=100000
BAR_25_DYNAMO=250000

[ -n "$GAME" ] || {
    echo "missing GAME: build one with this package first —" >&2
    echo "  ports/tools/launcher-game.sh build pneumaticcraft" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
command -v "$CTL" >/dev/null || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
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

launch() { # launch GAME [WORLD]
    local game=$1 world=${2:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch --game "$game" --world "$world" --seed 20261105 \
        --creative --headless --wait --resources "$RES" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# The block state's value of one property (`properties` object, or the `state` string).
side_state() { # X Y Z SIDE
    ctl block "$1" "$2" "$3" | jq -r --arg s "$4" '
        (.properties // {})[$s]
        // ((.state // "") | [scan("([a-z]+)=([a-z0-9]+)")] | map({key: .[0], value: .[1]}) | from_entries | .[$s])
        // ""'
}

assert_side() { # X Y Z SIDE VALUE
    local got
    got=$(side_state "$1" "$2" "$3" "$4")
    echo "  $1 $2 $3 $4=$got"
    [ "$got" = "$5" ] || fail "the block at $1 $2 $3 has $4=$got, expected $5"
}

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
    ctl command "execute in minecraft:the_end run fill $((X0 - 8)) 100 -18 $((X0 + 14)) 100 18 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 8)) 101 -18 $((X0 + 14)) 108 18 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
}

# `/pneumaticcraft info X Y Z`: parse the reply into PREFIX_AIR, PREFIX_BAR, ...
info() { # info X Y Z PREFIX
    say "pneumaticcraft info $1 $2 $3"
    ctl wait 14 >/dev/null
    local line
    line=$(ctl messages 14 | jq -r '.chat[]' | grep -m1 '^PC ' || true)
    [ -n "$line" ] || fail "no /pneumaticcraft info reply at $1 $2 $3"
    echo "  $line" >&2
    local body
    body=$(python3 "$PORT/tools/parse_info.py" "$line")
    eval "$(printf '%s\n' "$body" | sed "s/^\([A-Za-z_][A-Za-z0-9_]*\)=/$4_\1=/")"
    eval "[ \"\${${4}_OK:-0}\" = 1 ]" || fail "the info line did not parse: $line"
}

set_block() { # set_block X Y Z STATE
    ctl command "execute in minecraft:the_end run setblock $1 $2 $3 $4" >/dev/null
    ctl wait 4 >/dev/null
}

# The differential control: an unmodified game must know none of the ids.
control_leg() {
    echo "== control: the unmodified game does not know the mod's ids =="
    launch "$CONTROL_GAME" "$CONTROL_WORLD"
    arena
    ctl command "execute in minecraft:the_end run setblock $X0 101 0 pneumaticcraft:pressure_tube" >/dev/null || true
    ctl wait 4 >/dev/null
    local name
    name=$(block_name "$X0" 101 0)
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the tube id was known"
    for item in pressure_tube reinforced_pressure_tube advanced_pressure_tube; do
        ctl command "item replace entity @s hotbar.0 with pneumaticcraft:$item 1" >/dev/null || true
        ctl wait 3 >/dev/null
        ctl inventory | jq -e --arg id "pneumaticcraft:$item" \
            '[.hotbar[], .main[] | select(.item == $id)] | length == 0' >/dev/null \
            || fail "the control game has the $item item, so this game carries the mod"
    done
    echo "  without the package: no tube item, the tube block stayed $name"
    ctl quit >/dev/null 2>&1 || true
}

if [ "${CONTROL_ONLY:-0}" = 1 ]; then
    echo "== control: the same assertions on the game without the package =="
    # The differential as its own run: the *same* assertions on the game without the package.
    # Every one of them must fail, so this leg ends FAIL + EXIT:1 — the proof that a green
    # run above is the package's doing.
    [ -n "$CONTROL_GAME" ] || { echo "CONTROL_ONLY needs CONTROL_GAME" >&2; exit 2; }
    launch "$CONTROL_GAME" "$CONTROL_WORLD"
    arena
    set_block "$X0" 101 0 "pneumaticcraft:pressure_tube"
    [ "$(block_name "$X0" 101 0)" = "pneumaticcraft:pressure_tube" ] \
        || fail "the control game does not know pneumaticcraft:pressure_tube (as expected)"
    echo "CONTROL PASSED, which it must not"
    exit 0
fi
echo "== launch (The End arena, the launcher-built game) =="
if [ -n "$CONTROL_GAME" ]; then control_leg; fi
launch "$GAME"
arena

echo "== A: slice 1's pair still makes air from FE and shares it =="
# The compressor's pneumatic face is `facing`'s opposite: facing east faces west.
set_block "$((X0 + 2))" 101 -6 "pneumaticcraft:flux_compressor[facing=east,active=false]"
set_block "$((X0 + 1))" 101 -6 "pneumaticcraft:pneumatic_dynamo[facing=west,active=false]"
[ "$(block_name "$((X0 + 2))" 101 -6)" = "pneumaticcraft:flux_compressor" ] || fail "the compressor did not place"
[ "$(block_name "$((X0 + 1))" 101 -6)" = "pneumaticcraft:pneumatic_dynamo" ] || fail "the dynamo did not place"
say "pneumaticcraft energy $((X0 + 2)) 101 -6 $CAPACITY"
ctl wait 14 >/dev/null
info "$((X0 + 2))" 101 -6 A0
info "$((X0 + 1))" 101 -6 B0
ctl wait 100 >/dev/null
info "$((X0 + 2))" 101 -6 A1
info "$((X0 + 1))" 101 -6 B1
TICKS_A=$((A1_T - A0_T))
AIR_ADDED=$((A1_AIR + B1_AIR - A0_AIR - B0_AIR))
FE_SPENT=$((A0_FE - A1_FE))
[ "$TICKS_A" -gt 0 ] || fail "no ticks passed between the two reads"
[ "$AIR_ADDED" -ge $((AIR_A_TICK * TICKS_A - 5 * AIR_A_TICK - 40)) ] \
    || fail "the pair gained $AIR_ADDED mL in $TICKS_A ticks, expected about $((AIR_A_TICK * TICKS_A))"
[ "$AIR_ADDED" -le $((AIR_A_TICK * TICKS_A + 40)) ] \
    || fail "the pair gained $AIR_ADDED mL in $TICKS_A ticks, more than $((AIR_A_TICK * TICKS_A + 40))"
[ "$FE_SPENT" -ge $((FE_A_TICK * TICKS_A - 5 * FE_A_TICK - 40)) ] \
    || fail "the compressor spent $FE_SPENT FE in $TICKS_A ticks, expected about $((FE_A_TICK * TICKS_A))"
[ "$FE_SPENT" -le $((FE_A_TICK * TICKS_A + 40)) ] \
    || fail "the compressor spent $FE_SPENT FE in $TICKS_A ticks, more than $((FE_A_TICK * TICKS_A + 40))"
echo "  $TICKS_A ticks: air +$AIR_ADDED mL, FE -$FE_SPENT"

echo "== B: two tubes carry air to a dynamo across the network =="
# compressor[east face] - advanced tube - advanced tube - [west face]dynamo: 25 bar is the
# only pressure above the dynamo's 15 bar gate that both tubes can hold (a tier-two tube's).
F=$((X0 + 2))
T1=$((X0 + 3))
T2=$((X0 + 4))
DY=$((X0 + 5))
set_block "$F" 101 0 "pneumaticcraft:flux_compressor[facing=west,active=false]"
set_block "$T1" 101 0 "pneumaticcraft:advanced_pressure_tube[down=false,up=false,north=false,south=false,west=false,east=false]"
set_block "$T2" 101 0 "pneumaticcraft:advanced_pressure_tube[down=false,up=false,north=false,south=false,west=false,east=false]"
set_block "$DY" 101 0 "pneumaticcraft:pneumatic_dynamo[facing=east,active=false]"
[ "$(block_name "$F" 101 0)" = "pneumaticcraft:flux_compressor" ] || fail "the compressor did not place at $F"
[ "$(block_name "$T1" 101 0)" = "pneumaticcraft:advanced_pressure_tube" ] || fail "the first tube did not place"
[ "$(block_name "$T2" 101 0)" = "pneumaticcraft:advanced_pressure_tube" ] || fail "the second tube did not place"
[ "$(block_name "$DY" 101 0)" = "pneumaticcraft:pneumatic_dynamo" ] || fail "the dynamo did not place"
# 25 bar everywhere: the network is one pressure, so the dynamo runs above its 15 bar gate.
for pos in "$F::$BAR_25_FEEDER" "$T1::$BAR_25_TUBE" "$T2::$BAR_25_TUBE" "$DY::$BAR_25_DYNAMO"; do
    say "pneumaticcraft air ${pos%%::*} 101 0 ${pos##*::}"
done
# The compressor itself is fed: what the dynamo makes has to come from FE, across both tubes.
say "pneumaticcraft energy $F 101 0 $CAPACITY"
ctl wait 40 >/dev/null
info "$F" 101 0 B0
info "$T1" 101 0 B1
info "$T2" 101 0 B2
info "$DY" 101 0 B3
awk -v bar="$B3_BAR" 'BEGIN { exit !(bar + 0 > 23.5) }' \
    || fail "the network settled at $B3_BAR bar, not the 25 its four handlers were filled to"
awk -v bar="$B1_BAR" 'BEGIN { exit !(bar + 0 > 23.5) }' \
    || fail "the first tube reads $B1_BAR bar, not the network's pressure"
[ "$B1_AIR" -gt 0 ] || fail "the first tube holds no air ($B1_AIR mL)"
[ "$B2_AIR" -gt 0 ] || fail "the second tube holds no air ($B2_AIR mL)"
ctl wait 100 >/dev/null
info "$F" 101 0 B4
info "$T1" 101 0 B5
info "$T2" 101 0 B6
info "$DY" 101 0 B7
[ "$B7_FE" -gt "$B3_FE" ] || fail "the dynamo made no FE across the tubes (${B3_FE} -> ${B7_FE})"
awk -v bar="$B7_BAR" 'BEGIN { exit !(bar + 0 > 15) }' \
    || fail "the network fell to $B7_BAR bar, below the dynamo's gate, so its FE is not the tube's"
[ "$B5_AIR" -lt "$B1_AIR" ] || fail "the first tube did not feed the dynamo ($B1_AIR -> $B5_AIR mL)"
[ "$B6_AIR" -lt "$B2_AIR" ] || fail "the second tube did not feed the dynamo ($B2_AIR -> $B6_AIR mL)"
[ "$B4_FE" -lt "$B0_FE" ] || fail "the compressor spent no FE refilling the network"
TICKS_B=$((B4_T - B0_T))
[ $((B0_FE - B4_FE)) -ge $((FE_A_TICK * TICKS_B - 5 * FE_A_TICK - 200)) ] \
    || fail "the compressor spent only $((B0_FE - B4_FE)) FE in $TICKS_B ticks"
echo "  tubes ${B1_AIR}->${B5_AIR} and ${B2_AIR}->${B6_AIR} mL, FE made ${B3_FE}->${B7_FE}, compressor FE ${B0_FE}->${B4_FE} in $TICKS_B ticks"

echo "== C: the six connection booleans follow the network =="
assert_side "$T1" 101 0 west true
assert_side "$T1" 101 0 east true
assert_side "$T1" 101 0 north false
assert_side "$T1" 101 0 south false
assert_side "$T1" 101 0 up false
assert_side "$T2" 101 0 west true
assert_side "$T2" 101 0 east true
# A tube hung off one machine: one real connection, so the opposite side is shown too.
D1=$((X0 + 8))
D2=$((X0 + 9))
set_block "$D1" 101 8 "pneumaticcraft:flux_compressor[facing=west,active=false]"
set_block "$D2" 101 8 "pneumaticcraft:pressure_tube[down=false,up=false,north=false,south=false,west=false,east=false]"
ctl wait 10 >/dev/null
assert_side "$D2" 101 8 west true
assert_side "$D2" 101 8 east true
assert_side "$D2" 101 8 north false

echo "== D: a closed ring of tubes shares the air and keeps it =="
R1=$((X0 + 3))
R2=$((X0 + 4))
R3=$((X0 + 4))
R4=$((X0 + 3))
RZ1=4
RZ2=5
for spot in "$R1:$RZ1" "$R2:$RZ1" "$R3:$RZ2" "$R4:$RZ2"; do
    set_block "${spot%%:*}" 101 "${spot##*:}" \
        "pneumaticcraft:pressure_tube[down=false,up=false,north=false,south=false,west=false,east=false]"
done
ctl wait 10 >/dev/null
say "pneumaticcraft air $R1 101 $RZ1 4000"
ctl wait 120 >/dev/null
info "$R1" 101 "$RZ1" R1
info "$R2" 101 "$RZ1" R2
info "$R3" 101 "$RZ2" R3
info "$R4" 101 "$RZ2" R4
RING_TOTAL=$((R1_AIR + R2_AIR + R3_AIR + R4_AIR))
RING_MIN=$(printf '%s\n' "$R1_AIR" "$R2_AIR" "$R3_AIR" "$R4_AIR" | sort -n | head -1)
RING_MAX=$(printf '%s\n' "$R1_AIR" "$R2_AIR" "$R3_AIR" "$R4_AIR" | sort -n | tail -1)
if [ "$SELFTEST" = 1 ]; then
    echo "  (selftest: asserting the ring lost air)"
    [ "$RING_TOTAL" -lt 4000 ] || fail "selftest: the ring kept all $RING_TOTAL mL (as expected)"
else
    [ "$RING_TOTAL" -ge 3990 ] && [ "$RING_TOTAL" -le 4010 ] \
        || fail "the ring holds $RING_TOTAL mL of its 4,000: ${R1_AIR}/${R2_AIR}/${R3_AIR}/${R4_AIR}"
    [ "$RING_MIN" -ge 900 ] && [ "$RING_MAX" -le 1100 ] \
        || fail "the ring split unevenly: ${R1_AIR}/${R2_AIR}/${R3_AIR}/${R4_AIR}"
fi
echo "  ring: ${R1_AIR}/${R2_AIR}/${R3_AIR}/${R4_AIR} mL = $RING_TOTAL"

echo "== E: a dead-end tube vents what it is sent =="
say "pneumaticcraft energy $D1 101 8 $CAPACITY"
ctl wait 14 >/dev/null
info "$D2" 101 8 E0
ctl wait 200 >/dev/null
info "$D2" 101 8 E1
[ "$E1_AIR" -le 40 ] || fail "the dead-end tube holds $E1_AIR mL; it did not vent"
echo "  dead end after $((E1_T - E0_T)) ticks: $E1_AIR mL (the run's tubes held ${B1_AIR} mL)"

echo "== F: the three tubes have upstream's volumes and critical pressures =="
P1=$((X0 + 2))
P2=$((X0 + 4))
P3=$((X0 + 6))
for spot in "$P1|pneumaticcraft:pressure_tube|1000|7.000|7000" \
            "$P2|pneumaticcraft:reinforced_pressure_tube|1000|12.000|12000" \
            "$P3|pneumaticcraft:advanced_pressure_tube|4000|25.000|100000"; do
    IFS='|' read -r x id volume bar air <<<"$spot"
    set_block "$x" 101 -12 "$id[down=false,up=false,north=false,south=false,west=false,east=false]"
    [ "$(block_name "$x" 101 -12)" = "$id" ] || fail "$id did not place at $x 101 -12"
    say "pneumaticcraft air $x 101 -12 999999"
    ctl wait 14 >/dev/null
    info "$x" 101 -12 "T${x}"
    eval "got_volume=\${T${x}_VOLUME}"; eval "got_bar=\${T${x}_BAR}"; eval "got_air=\${T${x}_AIR}"
    [ "$got_volume" = "$volume" ] || fail "$id reads volume=$got_volume, expected $volume"
    [ "$got_air" = "$air" ] || fail "$id stores $got_air mL, expected its critical $air"
    [ "$got_bar" = "$bar" ] || fail "$id reads $got_bar bar, expected $bar"
    # A lone tube reaches no other handler, so it keeps what it was given.
    ctl wait 40 >/dev/null
    info "$x" 101 -12 "U${x}"
    eval "kept=\${U${x}_AIR}"
    [ "$kept" = "$air" ] || fail "$id alone lost air: $air -> $kept mL"
done
echo "  $P1 / $P2 / $P3: 1,000 mL at 7 bar, 1,000 mL at 12 bar, 4,000 mL at 25 bar; all kept"

echo "== G: the three tube items are in the game =="
slot=0
for item in pressure_tube reinforced_pressure_tube advanced_pressure_tube; do
    ctl command "item replace entity @s hotbar.$slot with pneumaticcraft:$item 3" >/dev/null
    slot=$((slot + 1))
done
ctl wait 6 >/dev/null
INVENTORY=$(ctl inventory)
for item in pressure_tube reinforced_pressure_tube advanced_pressure_tube; do
    echo "$INVENTORY" | jq -e --arg id "pneumaticcraft:$item" \
        '[.hotbar[], .main[] | select(.item == $id) | .count] | add == 3' >/dev/null \
        || fail "the $item item is not in the inventory"
done
echo "  gave pressure_tube, reinforced_pressure_tube, advanced_pressure_tube x3 each"

echo "== H: a block that is not a PneumaticCraft machine =="
set_block "$P3" 101 -14 "minecraft:stone"
say "pneumaticcraft info $P3 101 -14"
ctl wait 14 >/dev/null
NON_LINE=$(ctl messages 14 | jq -r '.chat[]' | grep -m1 'not a' || true)
[ -n "$NON_LINE" ] || fail "no 'not a ...' reply for a stone block"
echo "  $NON_LINE"

echo "PASS: the FE/air pair still runs; two tubes carry air to a dynamo that turns it back into FE; the six connection booleans follow the network; a closed ring splits 4,000 mL into four and loses none; a dead-end tube vents; the three tiers read 1,000 mL at 7 bar, 1,000 mL at 12 bar and 4,000 mL at 25 bar; and the three tube items are in the game"
