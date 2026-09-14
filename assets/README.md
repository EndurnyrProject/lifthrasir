# Lifthrasir Assets

## Converting GR2 actors

Convert the available GR2 models and associated action clips before running the
client or building a pak:

```bash
cargo run -p ro-to-lifthrasir-cli -- convert-gr2
# Optional: regenerate only one model
cargo run -p ro-to-lifthrasir-cli -- convert-gr2 --model guildflag90_1.gr2
```

The converter reads the effective GRF overlay configured in `assets/convert.toml`
and writes self-contained GLBs into `assets/data/models/3dmob/`. `--loader` selects
a different converter config; `--out` selects another output directory. Each GLB
contains its textures, skeleton and available idle/walk/attack/hit/death clips.
Models are processed in sorted order; failures are reported individually and
produce a nonzero exit status. Existing outputs are replaced only after the new
model passes validation.

The client loads `ro://models/3dmob/<model>.glb`, not raw GR2 or `.gr2.spr`/`.gr2.act`
paths. Missing optional action clips use idle; missing/corrupt GLBs are errors.
Guild flags use the owning guild's downloaded emblem when available and keep the
model's default texture otherwise.

The loose `assets/data` override takes effect immediately on the next client run.
For distribution, include these files with the existing pak build's
`--content-dir assets/data` option. **Packing does not convert models.** Bump the
pak content version on every rebuild; conversion itself does not replace the pak
or change the map format version.

Headless loader/animation validation of the generated files:

```bash
cargo test -p game-engine --test gr2_assets -- --ignored
```

## Generating the RON data

The game reads item and job metadata from `assets/data/ron/`. These files are
generated (gitignored) — regenerate them with the `convert` command:

```bash
# from the repo root
cargo run -p ro-to-lifthrasir-cli -- convert
```

Outputs:

- `assets/data/ron/item_data.ron` — item names, resources, descriptions, slot counts
- `assets/data/ron/job_data.ron` — PC display names + NPC/job sprite resource names

### Inputs

| Data | Source |
| --- | --- |
| Item names/descriptions | `assets/SystemEN/LuaFiles514/itemInfo.lua` |
| PC job display names | `assets/SystemEN/LuaFiles514/pcjobname.lub` |
| Job/NPC sprite names + `JOBID` map | GRFs (`jobidentity.lub`, `npcidentity.lub`, `jobname.lub`) |

English item and job names come from the on-disk **SystemEN** translation
project (zackdreaver/llchrisll), not the GRF. `assets/SystemEN/` is gitignored
and must be present — `convert` fails loudly with the missing path otherwise.
Job sprite names and the `JOBID`/`JTtbl` symbol map (which `pcjobname.lub`'s
keys resolve against) still come from the GRFs configured in `loader.toml`.

### Options

```bash
# pick a different loader config / output dir
cargo run -p ro-to-lifthrasir-cli -- convert --loader assets/loader.toml --out assets/data/ron

# regenerate only one dataset: "item" or "job"
cargo run -p ro-to-lifthrasir-cli -- convert --only item
```
