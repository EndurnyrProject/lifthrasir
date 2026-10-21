# Development

Run commands from the repository root. You'll need Rust and your own legitimate Ragnarok Online GRF files in `assets/`, plus a configured `assets/loader.toml`.

## Build the client

```bash
cargo run -p lifthrasir --features dev
```

The client needs its game data converted and packed first. Build the offline tools once; release builds make conversion much faster:

```bash
cargo build --release -p ro-to-lifthrasir-cli -p grf-utils
```

## Prepare game data

Convert the data tables, then convert maps to glTF. Map conversion is required: the runtime does not load maps from their original standalone files.

```bash
./target/release/ro-to-lifthrasir-cli convert
./target/release/ro-to-lifthrasir-cli convert-maps --force-models
```

`convert-maps` processes maps in sorted order, continues after individual failures, and exits non-zero if any map failed. Writes are atomic. To convert one map, use `convert-map --map <name>`.

Ground textures are pooled run-wide under `ground/tex/` (`--ground-dir`, default `assets/data/ground`) and referenced by absolute `ro://ground/tex/<name>.ktx2` URI; a map no longer carries its own `tex/` folder, and `convert-maps` removes a stale one left by older converters. Prop textures stay in `models/tex/`.

The runtime loads maps and props as converted GLBs. GND/RSW/RSM/RSM2 parsing is offline-only; the converted map embeds its original GAT data for runtime grounding, raycasts, and pathfinding. Water tile masks are baked during conversion. A missing or invalid map GLB fails to load.

Use `--force-models` after a `lif::MODEL_FORMAT_VERSION` change. Props are cached in `assets/data/models`, and stale models are rejected at runtime. Map and model format versions are independent: bump `lif::MAP_FORMAT_VERSION` for a map schema change and `lif::MODEL_FORMAT_VERSION` for a prop change.

If you need the standalone Hunter trap props or GR2 actors, convert them before packing:

```bash
./target/release/ro-to-lifthrasir-cli convert-props
./target/release/ro-to-lifthrasir-cli convert-gr2
```

The GR2 conversion reads `assets/convert.toml`. Both commands report individual failures and exit non-zero if any conversion failed.

## Build a pak

`pack` archives existing files; it does not run conversions. It excludes `.rsm`, `.rsm2`, `.gnd`, `.gat`, and `.rsw` files because the runtime does not read those formats directly. The manifest `content_version` is the pack timestamp (Unix milliseconds) by default, and `merge` only requires a patch to be newer than the main pak, so no manual bump is needed. Use `--content-version` only to reproduce an earlier build.

Write to a temporary file so a failed pack doesn't replace a working pak:

```bash
./target/release/grf-utils pack \
  --grf assets/en.grf --grf assets/data.grf \
  --content-dir assets/data \
  --out assets/lifthrasir.pak.new
mv assets/lifthrasir.pak.new assets/lifthrasir.pak
```

Earlier `--grf` flags win when paths conflict. `--content-dir` adds a folder at the pak root; `--data-folder` (optional) adds a retail data folder under its own name. Both take precedence over GRFs.

To patch an existing pak, build a patch pak and merge it while the game is closed. The patch file is consumed and the main pak is atomically replaced:

```bash
./target/release/grf-utils merge \
  --main assets/lifthrasir.pak --patch patch.pak
```
