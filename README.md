# Lifthrasir

A modern, cross-platform Ragnarok Online client implementation built with Rust, Bevy, and React.

## Overview

Lifthrasir is a reimplementation of the classic Ragnarok Online MMORPG client using modern technologies. The project leverages the Bevy game engine for high-performance 3D rendering and game logic, with a React-based UI overlay powered by Tauri for a native desktop experience.

## Technology Stack

- **Rust 2021 Edition** - Core game engine and backend
- **Bevy 0.17.2** - ECS-based game engine
- **Tauri v2** - Desktop application framework
- **React 18.3.1** - Frontend UI
- **TypeScript 5.9.3** - Type-safe UI development
- **Vite 7.1.9** - Frontend build tooling

## Prerequisites

- **Rust** (latest stable) - [Install here](https://rustup.rs/)
- **Node.js** (v18+) and npm - [Install here](https://nodejs.org/)
- **Ragnarok Online GRF Files** - You must provide your own legitimate GRF data files

### Required GRF Files

This client requires Ragnarok Online data files, which are proprietary to Gravity Co., Ltd. and are **NOT included** in this repository.

You must obtain these files legally from an official Ragnarok Online installation:
- `data.grf` - Main game data archive
- `rdata.grf` - Additional game resources (if applicable)

**Where to place GRF files:**
```
lifthrasir/
└── assets/
    ├── data.grf
    └── rdata.grf (optional)
```

## Getting Started

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/lifthrasir.git
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

### First-Time Users

Installers are unsigned in development builds. Users may see security warnings:
- **macOS**: Right-click the app → "Open" to bypass Gatekeeper
- **Windows**: Click "More info" → "Run anyway" on SmartScreen warning
- **Linux**: Mark the AppImage as executable: `chmod +x Lifthrasir.AppImage`

## Development Workflow

### Project Structure

```
lifthrasir/
├── game-engine/        # Core Bevy game engine (ECS systems, rendering, game logic)
├── src-tauri/          # Tauri integration layer (IPC bridge, window management)
├── web-ui/             # React frontend (UI components, authentication screens)
├── grf-utils/          # GRF file format utilities
└── assets/             # Game data files (user-provided GRF files go here)
```

### Common Commands

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint with Clippy
cargo clippy

# Check without building
cargo check

# Build game engine only
cd game-engine
cargo build

# Run UI in development
cd web-ui
npm run dev
```

## Contributing

Contributions are welcome! Please ensure:
1. Code follows Rust idioms and formatting (`cargo fmt`)
2. All tests pass (`cargo test`)
3. Clippy produces no warnings (`cargo clippy`)
4. Commits follow conventional commit format

## License

This project is an independent client implementation. Ragnarok Online and its assets are property of Gravity Co., Ltd. This software is provided for educational and interoperability purposes only.

## Legal Notice

This project is not affiliated with, endorsed by, or connected to Gravity Co., Ltd. or any official Ragnarok Online server. Users must provide their own legitimate game data files and comply with all applicable terms of service.

