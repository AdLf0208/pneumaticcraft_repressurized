#!/usr/bin/env python3
"""Assemble the slice-1 `content/` folder from the pack's PneumaticCraft jar.

The jar is the ATM10 file (`ports/_shared/licence-cache/pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar`,
SHA-1 `bced402c…864f`), kept as the licence and provenance evidence. Nothing here is invented:
the blockstates, block/item models, textures, lang file, loot tables and the pickaxe /
iron-tool tags come out of the jar, re-spelled only where 26.3 differs:

* a model's `neoforge:block/default` parent (a NeoForge model provider, absent from Horizon)
  becomes `minecraft:block/block`;
* the loot table's entry-level `conditions` list becomes one typed `condition` per pool
  (the 26.3 shape: `"condition": {"type": "minecraft:survives_explosion"}`), and the
  `copy_components` functions go — the components they name (`pneumaticcraft:air`,
  `:be_saved_rc`, `:upgrades`) are not registered by this slice.
* the item model definition 26.3 wants (`assets/<id>/items/<name>.json`) is written here.

The two block declarations are written here too (a vanilla `minecraft:stone` template with the
declared `facing`/`active` state properties upstream's blockstate JSON uses), and a creative tab.

Upstream's recipes are **not** carried: they name `compressed_iron_gear`,
`printed_circuit_board`, `turbine_rotor` and `advanced_pressure_tube`, none of which this slice
ports, so any recipe would be uncraftable and an unknown *item* ingredient is a load-time check
failure (see CONTENT.md).

Rerun it to rebuild `assets/` and `data/` at the package root (it deletes its own
generated folders first):

    python3 tools/assemble_content.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/licence-cache/pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar"
# The package root since the SDK format (2026-11-18): assets/ and data/ sit beside
# horizon-mod.toml, which is the only manifest — this script no longer writes one.
CONTENT = ROOT
MOD = "pneumaticcraft"
# The pack jar's SHA-1 (the same file `licence-precheck.csv` records).
JAR_SHA1 = "bced402c1224d948554b1e8fcca2aa002c00864f"

BLOCKS = ("flux_compressor", "pneumatic_dynamo")
# Slice 2: the three pressure tubes. They are connecting blocks on the chorus plant's
# `PipeBlock`: six `down/up/north/south/east/west` booleans, a 4-pixel arm, and the block
# state `thickness` shapes.
TUBES = ("pressure_tube", "reinforced_pressure_tube", "advanced_pressure_tube")
TUBE_TAG = "pressure_tubes"

# Carried byte-for-byte: the jar's own assets and data.
CARRIED = [
    *(f"assets/{MOD}/blockstates/{name}.json" for name in BLOCKS),
    *(f"assets/{MOD}/models/block/{name}_{state}.json" for name in BLOCKS for state in ("off", "on")),
    *(f"assets/{MOD}/models/item/{name}.json" for name in BLOCKS),
    *(f"assets/{MOD}/textures/pnc_model/{name}_{state}.png" for name in BLOCKS for state in ("off", "on")),
    *(f"assets/{MOD}/textures/pnc_model/{name}_on.png.mcmeta" for name in BLOCKS),
    f"assets/{MOD}/lang/en_us.json",
    *(f"data/{MOD}/loot_table/blocks/{name}.json" for name in BLOCKS),
    "data/minecraft/tags/block/mineable/pickaxe.json",
    "data/minecraft/tags/block/needs_iron_tool.json",
    # Slice 2: the tubes' art and their two Blockbench parts — the up-facing arm and the
    # face plate an unconnected side shows (`PressureTubeModelLoader` loads exactly these).
    *(f"assets/{MOD}/textures/pnc_model/{name}.png" for name in TUBES),
    *(f"assets/{MOD}/models/block/{name}_connected.json" for name in TUBES),
    f"assets/{MOD}/models/block/pressure_tube_inv.json",
    *(f"assets/{MOD}/models/item/{name}.json" for name in TUBES),
    *(f"data/{MOD}/loot_table/blocks/{name}.json" for name in TUBES),
]

# `defaultProps()`: `strength(3.0, 10.0).sound(METAL).mapColor(METAL)`; the
# template carries the hardness, the declared properties carry upstream's states.
PROPERTIES = {
    "hardness": 3.0,
    "explosion_resistance": 10.0,
    "requires_tool": True,
    "sounds": "minecraft:iron_block",
}

BLOCK_DECLARATIONS = {
    "flux_compressor": {
        "template": "minecraft:stone",
        "state_properties": {
            "facing": {"type": "enum", "values": ["north", "south", "east", "west"]},
            "active": {"type": "boolean"},
        },
        "default_state": {"facing": "north", "active": "false"},
        "properties": PROPERTIES,
        "item": True,
    },
    "pneumatic_dynamo": {
        "template": "minecraft:stone",
        "state_properties": {
            "facing": {
                "type": "enum",
                "values": ["north", "south", "east", "west", "up", "down"],
            },
            "active": {"type": "boolean"},
        },
        "default_state": {"facing": "north", "active": "false"},
        "properties": PROPERTIES,
        "item": True,
    },
}

ITEM_DEFINITION = {
    name: {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{name}"}}
    for name in BLOCKS
}
ITEM_DEFINITION.update(
    {name: {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{name}"}} for name in TUBES}
)

# The tubes: `defaultProps().noOcclusion()` upstream, and the chorus plant's own six
# connecting booleans (the template carries them, so no `state_properties` here).
TUBE_PROPERTIES = {**PROPERTIES, "occludes": False}

BLOCK_DECLARATIONS.update(
    {
        name: {
            "template": "minecraft:chorus_plant",
            "connects_to": f"#{MOD}:{TUBE_TAG}",
            "thickness": 4.0,
            "properties": TUBE_PROPERTIES,
            "item": True,
        }
        for name in TUBES
    }
)

# The arm that is drawn for a connected side, and the face plate for a free one: both are the
# jar's own Blockbench parts, in the UP orientation, rotated by the blockstate
# (`PressureTubeGeometry.ROTATIONS`: X180 for down, X0 for up, X90 for north, X90/Y180 for
# south, X90/Y270 for west, X90/Y90 for east).
TUBE_ROTATIONS = {
    "down": {"x": 180},
    "up": {},
    "north": {"x": 90},
    "south": {"x": 90, "y": 180},
    "west": {"x": 90, "y": 270},
    "east": {"x": 90, "y": 90},
}


def tube_blockstate(name: str) -> dict:
    """Every side draws the arm when it connects and the face plate when it does not."""
    multipart = []
    for side, rotation in TUBE_ROTATIONS.items():
        for connected in ("true", "false"):
            part = f"{name}_connected" if connected == "true" else f"{name}_collar"
            multipart.append(
                {
                    "when": {side: connected},
                    "apply": {"model": f"{MOD}:block/{part}", **rotation},
                }
            )
    return {"multipart": multipart}

TAB = {
    "title": "itemGroup.pneumaticcraft",
    "icon": f"{MOD}:flux_compressor",
    "items": [f"{MOD}:{name}" for name in (*BLOCKS, *TUBES)],
}



def upgrade_model(model: dict) -> dict:
    """A NeoForge model provider is not part of Horizon's asset root."""
    if model.get("parent") == "neoforge:block/default":
        model["parent"] = "minecraft:block/block"
    return model


def upgrade_loot(loot: dict) -> dict:
    """26.3's one typed condition per pool, and no unregistered components."""
    for pool in loot.get("pools", []):
        pool.pop("conditions", None)
        pool["condition"] = {"type": "minecraft:survives_explosion"}
        for entry in pool.get("entries", []):
            entry.pop("functions", None)
    return loot


def write(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        path.write_bytes(data)
    else:
        path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def main() -> None:
    raw = JAR.read_bytes()
    digest = hashlib.sha1(raw).hexdigest()
    if digest != JAR_SHA1:
        raise SystemExit(f"jar SHA-1 {digest} is not the pack record {JAR_SHA1}")
    archive = zipfile.ZipFile(JAR)
    for folder in ("assets", "data"):
        shutil.rmtree(CONTENT / folder, ignore_errors=True)

    for name in CARRIED:
        data = archive.read(name)
        if "/models/block/" in name:
            data = (json.dumps(upgrade_model(json.loads(data)), indent=2) + "\n").encode()
        elif "/loot_table/blocks/" in name:
            data = (json.dumps(upgrade_loot(json.loads(data)), indent=2) + "\n").encode()
        write(CONTENT / name, data)

    for block, declaration in BLOCK_DECLARATIONS.items():
        write(CONTENT / f"data/{MOD}/horizon/block/{block}.json", declaration)
        write(CONTENT / f"assets/{MOD}/items/{block}.json", ITEM_DEFINITION[block])
    # The tubes' face plates: the jar's one `pressure_tube_disconnected` part (a 4x4 plate on
    # a free side), its texture pointed at the tube it belongs to.
    collar = json.loads(
        archive.read(f"assets/{MOD}/models/block/pressure_tube_disconnected.json")
    )
    for name in TUBES:
        plate = json.loads(json.dumps(collar))
        plate["textures"] = {
            "particle": f"{MOD}:pnc_model/{name}",
            "pipe": f"{MOD}:pnc_model/{name}",
        }
        write(CONTENT / f"assets/{MOD}/models/block/{name}_collar.json", plate)
        write(CONTENT / f"assets/{MOD}/blockstates/{name}.json", tube_blockstate(name))
    write(
        CONTENT / f"data/{MOD}/tags/block/{TUBE_TAG}.json",
        {"values": [f"{MOD}:{name}" for name in TUBES]},
    )
    write(CONTENT / f"data/{MOD}/horizon/creative_tab/pneumaticcraft.json", TAB)

    files = sorted(
        path
        for folder in ("assets", "data")
        for path in (CONTENT / folder).rglob("*")
        if path.is_file()
    )
    print(f"{len(files)} carried files under {CONTENT}")
    for path in files:
        print(" ", path.relative_to(CONTENT))


if __name__ == "__main__":
    main()
