#!/usr/bin/env bash
# Package both artifacts:
#  - the source-only .hnmod (tools/package.py, deterministic bytes, in the port root)
#  - the .hmod via the pin's own cargo-horizon-mod (build + check), from content/
# The built .hmod is also copied into the bundle's data/ so one .hnmod carries the content
# (the AE2 pattern); tools/package.py refuses a stale embedded copy.
# Run inside a build slot (it compiles the packaging tool):
#   ports/tools/build-slot.sh run --job pc-pack --port ports/pneumaticcraft \
#       --step package -- env CARGO_INCREMENTAL=0 ports/pneumaticcraft/tools/package.sh
set -euo pipefail

PORT=$(cd "$(dirname "$0")/.." && pwd)
HORIZON=${HORIZON:-$PORT/../_shared/pins/horizon-1d63e0a}
LOGS=$PORT/target/ladder
mkdir -p "$LOGS"
cd "$PORT"

CARGO_TARGET_DIR=$PORT/target/carry-tools
export CARGO_TARGET_DIR
(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod build) \
    2>&1 | tee "$LOGS/package-hmod.log"
cp content/target/horizon-mod/pneumaticcraft.hmod content/pneumaticcraft.hmod
(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod check pneumaticcraft.hmod) \
    2>&1 | tee -a "$LOGS/package-hmod.log"

cp content/pneumaticcraft.hmod mixins/pneumaticcraft/data/pneumaticcraft-content.hmod
python3 tools/package.py | tee "$LOGS/package-hnmod.log"
cp target/pneumaticcraft-0.1.0.hnmod pneumaticcraft.hnmod

echo "== artifacts =="
for f in content/pneumaticcraft.hmod pneumaticcraft.hnmod; do
    [ -f "$f" ] || { echo "missing $f"; exit 1; }
    size=$(stat -c %s "$f")
    sha=$(sha256sum "$f" | cut -d' ' -f1)
    entries=$(python3 -c "import zipfile,sys; print(len(zipfile.ZipFile(sys.argv[1]).namelist()))" "$f")
    echo "$f size=$size sha256=$sha entries=$entries"
done
