<p align="center">
  <img width="300" height="300" src="https://iili.io/KbXkCeR.md.png">
</p>

[Discord Link](https://discord.gg/mcae5Gh6Wg)


# Lifthrasir

A modern, cross-platform Ragnarok Online client implementation built with Rust and Bevy.

## Overview

I always wanted to build this stuff, and since i wanted to learn Rust, why not? Will this ever be fully playable? Probably not,
maybe, who knows?

### Project Architecture

The architecture is fairly simple, everything is build following the Entity Component System (ECS) paradigm using Bevy as the game engine.
The UI is built natively with Bevy.

## Prerequisites

- **Rust** (latest stable)
- **Ragnarok Online GRF Files** - You must provide your own legitimate GRF data files

### Required GRF Files

This client requires Ragnarok Online data files, which are proprietary to Gravity Co., Ltd. and are **NOT included** in this repository.

## Getting Started

### 1. Clone the Repository

```bash
git clone git@github.com:EndurnyrProject/lifthrasir.git
cd lifthrasir
```

### 2. Install Dependencies

```bash
cargo build
```

### 3. Add Your GRF Files

Place your Ragnarok Online GRF files in the `assets/` directory as described above.

### 4. Generate Derived Assets

```bash
cargo run -p ro-to-lifthrasir-cli -- convert
```

### 5. Run

```bash
cargo run -p lifthrasir
```

## Building for Distribution

```bash
cargo build --release
```

## Server

For now, its working only with [Aesir](https://github.com/EndurnyrProject/aesir)

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
