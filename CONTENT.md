# PneumaticCraft slice 1 — content carry

> **Paths since the SDK format (0.2.0).** The carried files live at the package root —
> `assets/pneumaticcraft/…` and `data/…` — not under `content/`, and the root
> `horizon-mod.toml` is the only manifest (`content/horizon-mod.toml` is retired evidence).
> `tools/assemble_content.py` writes them there and reproduces all 24 files byte for byte.
> The retired copy is `../_shared/evidence/pneumaticcraft/retired-hnmod-0.1.0/content/`.

Built by `tools/assemble_content.py` from the pack jar
(`ports/_shared/licence-cache/pneumaticcraft-repressurized-8.2.24-mc1.21.1.jar`,
SHA-1 `bced402c…`, the file `licence-precheck.csv` records). Everything below comes out of that
jar; only the re-spellings 26.3 needs are made.

## Carried byte-for-byte (21 files)

| What | Jar path |
|---|---|
| blockstates | `assets/pneumaticcraft/blockstates/{flux_compressor,pneumatic_dynamo}.json` |
| block models | `assets/pneumaticcraft/models/block/{flux_compressor,pneumatic_dynamo}_{off,on}.json` |
| item models | `assets/pneumaticcraft/models/item/{flux_compressor,pneumatic_dynamo}.json` |
| textures | `assets/pneumaticcraft/textures/pnc_model/{flux_compressor,pneumatic_dynamo}_{off,on}.png` + the two `_on.png.mcmeta` |
| lang | `assets/pneumaticcraft/lang/en_us.json` (whole, 2,274 keys, 267 KB) |
| loot tables | `data/pneumaticcraft/loot_table/blocks/{flux_compressor,pneumatic_dynamo}.json` |
| tags | `data/minecraft/tags/block/mineable/pickaxe.json`, `…/needs_iron_tool.json` |

## Re-spelled for 26.3

1. **Model parent.** Both `_off` models name `neoforge:block/default` (a NeoForge model provider);
   Horizon's asset root has no `neoforge:` namespace, so the parent becomes
   `minecraft:block/block` (the vanilla `block/block` parent, which the prepared 26.3 root ships).
2. **Loot tables.** The jar's entry-level `conditions` list becomes one typed `"condition":
   {"type": "minecraft:survives_explosion"}` per pool (the 26.3 shape; the 1.21.1 plural list
   silently evaluates false). The `copy_name` / `copy_components` functions are removed: the
   components they name (`pneumaticcraft:air`, `:be_saved_rc`, `:upgrades`) are not registered by
   this slice.
3. **Item model definitions.** 26.3 reads `assets/<id>/items/<name>.json`:
   `{"model": {"type": "minecraft:model", "model": "pneumaticcraft:item/<name>"}}`, over the
   carried `models/item/<name>.json`.

## Written here (3 files + the manifest)

- **Block declarations** (`data/pneumaticcraft/horizon/block/`): a `minecraft:stone` template
  (full cube, `strength(3.0, 10.0)` = upstream's `defaultProps()`, metal sound, iron tool) with
  the declared state properties the blockstate JSON uses —
  `facing` 4 values for the compressor, 6 for the dynamo (upstream's coil is placed against the
  clicked face), and `active` (boolean, default false). 26.3 lets a block declare properties its
  template does not have, and these two blocks are not vanilla-shaped.
- **Creative tab** (`data/pneumaticcraft/horizon/creative_tab/pneumaticcraft.json`): upstream's
  `itemGroup.pneumaticcraft`.
- **`horizon-mod.toml`**: id `pneumaticcraft`, version `0.1.0` (the port's own version), API 0.2,
  `side = "both"`, capability `registries`.

## Deliberately not carried

- **Recipes.** The jar's recipes name `pneumaticcraft:compressed_iron_gear`,
  `printed_circuit_board`, `turbine_rotor` and `advanced_pressure_tube`, none of which this slice
  ports. An unknown *item* ingredient fails the content check (an unknown *tag* would silently
  never match), so both recipes are left for the slice that brings the item chain. The blocks are
  reachable in creative and by `/give`.
- **Advancements** for the two blocks: they depend on the item chain above.
- **Patchouli book pages**, sounds, particles and the block entity renderers: not part of this
  slice; the block models are static like upstream's.

## Size and hashes

The 0.1.0 content package was `content/pneumaticcraft.hmod` — 131,324 B, SHA-256
`0cae9fda259bc50b9a7e167636a38103d129b5ed4ead61e09a9b0ca2daf9f205`, 25 entries, `no code: content
only`, carried inside the `.hnmod` (the AE2 pattern). Both are retired evidence. The same 24 files
are now part of the one flat package, `pneumaticcraft.hmod` (0.2.0).
