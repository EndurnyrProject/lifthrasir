<p align="center">
  <img width="300" height="300" src="https://iili.io/KbXkCeR.md.png">
</p>

[![](https://dcbadge.limes.pink/api/server/https://discord.gg/xGq832HYey)](https://discord.gg/xGq832HYey)

# Lifthrasir

A modern, cross-platform Ragnarok Online client implementation built with Rust and Bevy.

## Overview and Objective

I always wanted to build this stuff, and since i wanted to learn Rust, why not? Will this ever be fully playable? Probably not,
maybe, who knows?

The objetive here is most fun, i don't want to build a 1-to-1 ragnarok client copy, i want some liberty to add new stuff, redo some stuff
i didn't like, while trying to keep the same feeling.

## Project Architecture and Functionalities

I'm trying to keep it as close to the Bevy ECS recommended architecture, the basic concepts are: 

1. Follow the ECS pattern, where systems operate on components attached to entities.
2. Keep the network and game logic separate, albeit i don't want to build compatibility for other servers, do not keep people from doing it.

## Prerequisites

- **Rust** (latest stable)
- **Ragnarok Online GRF Files** - You must provide your own legitimate GRF data files

### Required Files

This client requires Ragnarok Online data files, which are proprietary to Gravity Co., Ltd. and are **NOT included** in this repository.

## Getting Started

```bash
git clone git@github.com:EndurnyrProject/lifthrasir.git
# Add your *.grf to the assets folder, configure the loader.toml

# Build the offline tooling once -- release is dramatically faster here
cargo build --release -p ro-to-lifthrasir-cli -p grf-utils

# 1. Convert the data tables (jobs, items, skills, ...)
./target/release/ro-to-lifthrasir-cli convert

# 2. Convert every map to glb. REQUIRED -- the client loads maps only from
#    converted glbs, so an unconverted map is a hard startup failure.
#    Takes a while: it converts each map's terrain, lighting, water and props.
./target/release/ro-to-lifthrasir-cli convert-maps --force-models

# 3. Generate your pak file. Bump --content-version on every rebuild; it is
#    enforced monotonic. Writing to .new first keeps a failed pack from
#    destroying a working pak.
./target/release/grf-utils pack \
  --grf assets/en.grf --grf assets/data.grf \
  --content-dir assets/data \
  --out assets/lifthrasir.pak.new --content-version 1
mv assets/lifthrasir.pak.new assets/lifthrasir.pak

# 4. Run the app on dev mode
cargo run -p lifthrasir --features dev
```

### Notes on the asset pipeline

- **Map conversion is not optional.** The runtime has no Ragnarok map reader:
  it does not parse GND, GAT, RSW, RSM or RSM2. Maps load exclusively from
  `data/maps/<map>/<map>.glb`, produced by `convert-maps`. Requesting a map
  that has not been converted fails loudly rather than degrading.
- **`--force-models` matters when the glb format version changes.** Props are
  cached in `assets/data/models` and skipped if already present, so after a
  format bump you must force a reconvert or your maps will reference stale
  models the runtime rejects.
- **`convert-maps` continues past failures.** It logs each failing map with its
  error, converts the rest, prints converted/failed counts and exits non-zero
  if anything failed. Writes are atomic, so a failed map leaves no partial glb.
  To (re)convert a single map, use `convert-map --map <name>`.
- **`pack` never converts anything.** It only archives what is already on disk,
  so always convert before packing. It skips Ragnarok's native map and model
  formats (`.rsm`, `.gnd`, `.gat`, `.rsw`) since nothing reads them at runtime,
  and reports how many entries it excluded.
- **Patching an existing install:** build a patch pak the same way, then
  `grf-utils merge --main assets/lifthrasir.pak --patch patch.pak` with the game
  closed. The patch file is consumed and the main pak atomically replaced.

## Server Side

### Does it work with rAthena?

Nope, and its not my plan to make it work, only with [Aesir](https://github.com/EndurnyrProject/aesir), the
engine protocol is agnostic though, feel free to implement the protocol
for rAthena if you want.

## Contributing

Contributions are welcome! Please ensure:
1. Code follows Rust idioms and formatting (`cargo fmt`)
2. All tests pass (`cargo test`)
3. Clippy produces no warnings (`cargo clippy`)
4. Commits follow conventional commit format

## Thank you

I shamelessly took a lot of code and ideas from these amazing projects:

- [RagnarokRebuildTcp](https://github.com/Doddler/RagnarokRebuildTcp)  
- [BrowEdit3](https://github.com/Borf/BrowEdit3)  
- [GRFEditor](https://github.com/Tokeiburu/GRFEditor)  
- [Korangar](https://github.com/vE5li/korangar)  

## Legal Notice

This project is not affiliated with, endorsed by, or connected to Gravity Co., Ltd. or any official Ragnarok Online server. Users must provide their own legitimate game data files and comply with all applicable terms of service.
Gravity pls don't strike me :(
