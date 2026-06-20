# AGENTS.md - Lifthrasir Codebase Documentation

## Skills to use

- Load the bevy-cheatbook skill
- When working with UI, load the bevy-enhanced-ui skill
- Always Load the ponytail skill, it helps you to be more token efficient

---

## Project Overview

**Lifthrasir** is a Ragnarok Online client implementation written in Rust using the Bevy game engine with a native Bevy UI. The project aims to recreate the classic MMORPG client while leveraging modern technologies for cross-platform compatibility, performance, and maintainability.

### Key Features
- Full support for Ragnarok Online file formats (GRF, GND, GAT, RSW, RSM, SPR, ACT)
- 3D terrain rendering with proper coordinate system translation
- Character rendering with equipment and animation systems
- Authentication and character management
- Native UI built with Bevy

---

## Technology Stack

### Core Technologies
- **Rust (Edition 2021)**: Primary programming language for game engine
- **Bevy 0.18.1**: ECS-based game engine for rendering, game logic, and UI

### Key Paradigms
- **Entity Component System (ECS)**: Bevy's core architecture pattern
- **Clean Architecture**: Layered design with clear separation of concerns
- **Domain-Driven Design (DDD)**: Business logic organized by domain concepts
- **Event-Driven Architecture**: Communication via Bevy events


---

## Architecture

### Workspace Structure
The project is organized as a Cargo workspace:

```
lifthrasir/
├── game-engine/        # Core game engine (Bevy ECS)
├── lifthrasir-ui/      # Native Bevy UI components
├── lifthrasir/         # Binary entry point
└── grf-utils/          # GRF archive utilities
```


### Building

```bash
# Build game engine only
cd game-engine
cargo build

# Build entire workspace
cargo build

# Release build
cargo build --release
```

### Running

```bash
cargo run -p lifthrasir
```

### Generating network protobuf types

The client talks to the aesir account server over QUIC using protobuf (`bevy_quinnet` + `prost`). The Rust types are generated from aesir's canonical `aesir.proto` and committed at `game-engine/src/infrastructure/networking/quic/proto/aesir.net.rs`. Re-run this whenever that schema changes:

```bash
cargo run -p ro-to-lifthrasir-cli -- gen-proto \
  --src <aesir>/apps/commons/proto \
  --out game-engine/src/infrastructure/networking/quic/proto/aesir.net.rs
```

This uses the pure-Rust `protox` compiler, so no system `protoc` is required. Commit the regenerated file.

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test <test_name>
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Lint with Clippy
cargo clippy

# Check without building
cargo check
```

---

## Development Guidelines

### When Adding New Features

1. **Check existing patterns**: Look for similar features before implementing
2. **Follow layer separation**: Domain logic separate from infrastructure
3. **Write tests**: Add tests for new domain logic

### Code Style

1. Prevent nesting of ifs, prefer a more functional style and early returns.
2. Critical systems should not have fallbacks, they should fail loudly.
3. Always check the libraries usage and examples using the Context 7 Tool
4. Think before writing: Is there a simpler way to achieve this?
5. Keep functions simple and pure, prevent the creation of god functions with several parameters.
6. Prefer splitting code in modules instead of god files.
7. Always consult the bevy cheatbook https://bevy-cheatbook.github.io/
8. Consult bevy examples, they are very helpful https://github.com/bevyengine/bevy/tree/latest/examples#examples
9. Also check the bevy documentation for up-to-date function https://docs.rs/bevy/latest/bevy/
