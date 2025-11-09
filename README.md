<p align="center">
  <img width="300" height="300" src="https://iili.io/KbXkCeR.md.png">
</p>

# Lifthrasir

A modern, cross-platform Ragnarok Online client implementation built with Rust, Bevy, and React.

## Overview

I always wanted to build this stuff, and since i wanted to learn Rust, why not? Will this ever be fully playable? Probably not, 
maybe, who knows? 

### Project Architecture

The architecture is fairly simple, everything is build following the Entity Component System (ECS) paradigm using Bevy as the game engine.
However, since Bevy still doesn't provide a good way of building UIs, i got the genially idiotic idea of using Tauri, which allows us to
use React for building the UI, and communicate with the Bevy game engine using IPC. Which works, but boy its a pain in the ass.

## Prerequisites

- **Rust** (latest stable)
- **Node.js** (v18+) and npm
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
# Install Tauri CLI globally (if not already installed)
cargo install tauri-cli

# Install Rust dependencies (from project root)
cargo build

# Install UI dependencies
cd web-ui
npm install
cd ..
```

### 3. Add Your GRF Files

Place your Ragnarok Online GRF files in the `assets/` directory as described above.

### 4. Run in Development Mode

```bash
cd src-tauri
cargo tauri dev
```

## Building for Distribution

To create distributable installers for all platforms:

```bash
cargo tauri build
```

This will create platform-specific packages in `src-tauri/target/release/bundle/`:
- **macOS**: `.app` bundle and `.dmg` installer
- **Windows**: `.exe` executable and `.msi` installer
- **Linux**: `.AppImage`, `.deb`, and `.rpm` packages

## Contributing

Contributions are welcome! Please ensure:
1. Code follows Rust idioms and formatting (`cargo fmt`)
2. All tests pass (`cargo test`)
3. Clippy produces no warnings (`cargo clippy`)
4. Commits follow conventional commit format

## Thank you

I shamelessly took a lot of code and ideas from these amazing projects:

[RagnarokRebuildTcp](https://github.com/Doddler/RagnarokRebuildTcp)  
[BrowEdit3](https://github.com/Borf/BrowEdit3)  
[GRFEditor](https://github.com/Tokeiburu/GRFEditor)  

## Legal Notice

This project is not affiliated with, endorsed by, or connected to Gravity Co., Ltd. or any official Ragnarok Online server. Users must provide their own legitimate game data files and comply with all applicable terms of service. 
Gravity pls don't strike me :(

