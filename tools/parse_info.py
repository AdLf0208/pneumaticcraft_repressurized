#!/usr/bin/env python3
"""Turn one `/pneumaticcraft info` line into shell assignments for `tools/e2e.sh`.

The line is `support.rs`'s `readout`:

    PC pneumaticcraft:flux_compressor [302, 101, 0] air=1600 volume=10000 bar=0.160
       fe=96000 temp=297.2 eff=100 active=true t=1240

Prints `ok=1` and one assignment per field, or `ok=0` when the line is not a
readout (a block with no slice-1 block entity answers another sentence).
"""
from __future__ import annotations

import re
import sys

PATTERN = re.compile(
    r"^PC (?P<block>\S+) \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\] "
    r"air=(?P<air>-?\d+) volume=(?P<volume>\d+) bar=(?P<bar>-?[\d.]+) "
    r"fe=(?P<fe>-?\d+) temp=(?P<temp>-?[\d.]+) eff=(?P<eff>-?\d+) "
    r"active=(?P<active>\w+) t=(?P<t>-?\d+)$"
)


def main() -> int:
    line = " ".join(sys.argv[1:]).strip()
    match = PATTERN.match(line)
    if not match:
        print("OK=0")
        return 0
    fields = match.groupdict()
    print("OK=1")
    print(f"BLOCK={fields['block']}")
    for key in ("air", "volume", "fe", "eff", "t"):
        print(f"{key.upper()}={fields[key]}")
    print(f"BAR={fields['bar']}")
    print(f"TEMP={fields['temp']}")
    print(f"ACTIVE={fields['active']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
