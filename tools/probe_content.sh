#!/usr/bin/env bash
# Diagnostic: why does the content package's block id not register?
# Launches the game twice — once with only the .hnmod (embedded content) and
# once with the content folder as a dev mod — and dumps the log lines that name
# the mod, the content root, a declaration or a refusal.
set -Eeuo pipefail
export LC_ALL=C

PORT=$(cd "$(dirname "$0")/.." && pwd)
CLONE=$(cd "$PORT/../.." && pwd)
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/pneumaticcraft-probe.sock}

SCRATCH=$(mktemp -d /tmp/opencode/pneumaticcraft-probe-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"

ctl() { "$CTL" --socket "$SOCK" "$@"; }

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$SCRATCH" 2>/dev/null || true
    echo "EXIT:$status"
}
trap cleanup EXIT

probe() { # probe WORLD MODE
    local world=$1 mode=$2
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    if [ "$mode" = dev ]; then args+=(--dev-mod "$PORT/content"); fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    ctl wait-ready 120
    ctl wait 40
    ctl command "execute in minecraft:the_end run tp @s 320.5 104 0.5" >/dev/null || true
    for _ in $(seq 1 60); do
        ctl wait 5 >/dev/null
        [ "$(ctl block 320 100 0 | jq -r '.loaded // false')" = true ] && break
    done
    ctl command "execute in minecraft:the_end run fill 318 100 -4 326 100 4 minecraft:stone" >/dev/null || true
    ctl command "execute in minecraft:the_end run tp @s 320.5 101 0.5" >/dev/null || true
    ctl wait 20 >/dev/null
    ctl command "execute in minecraft:the_end run setblock 322 101 0 pneumaticcraft:flux_compressor[facing=east,active=false]" >/dev/null || true
    ctl wait 4 >/dev/null
    echo "  mode=$mode block=$(ctl block 322 101 0 | jq -r '.name // .block // empty')"
    echo "  -- chat --"
    ctl messages 10 | jq -r '.chat[]' | tail -5
    echo "  -- log lines naming the mod or a refusal --"
    ctl logs --lines 3000 | grep -iE "pneumaticcraft|content root|left out|refus|declaration|invalid|unknown block|template|content from" | tail -40
    ctl quit >/dev/null 2>&1 || true
}

echo "== embedded (the .hnmod only) =="
probe ProbeEmbedded embedded
rm -rf "$SCRATCH/.horizon-cache"
echo "== dev mod (the content folder) =="
probe ProbeDev dev
echo "PASS: probe finished"
