# Lifthrasir Assets

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
