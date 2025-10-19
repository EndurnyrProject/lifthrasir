# AGENTS.md - Lifthrasir Codebase Documentation

---

## Project Overview

**Lifthrasir** is a Ragnarok Online client implementation written in Rust using the Bevy game engine with a React-based UI powered by Tauri. The project aims to recreate the classic MMORPG client while leveraging modern technologies for cross-platform compatibility, performance, and maintainability.

### Key Features
- Full support for Ragnarok Online file formats (GRF, GND, GAT, RSW, RSM, SPR, ACT)
- 3D terrain rendering with proper coordinate system translation
- Character rendering with equipment and animation systems
- Authentication and character management
- Modern UI built with React overlaying the game world

---

## Technology Stack

### Core Technologies
- **Rust (Edition 2021)**: Primary programming language for game engine
- **Bevy 0.16.1**: ECS-based game engine for rendering and game logic
- **Tauri v2**: Desktop application framework for UI integration
- **React 18.3.1**: Frontend UI framework
- **TypeScript 5.6.2**: Type-safe JavaScript for UI code
- **Vite 6.0.3**: Frontend build tool and dev server

### Key Paradigms
- **Entity Component System (ECS)**: Bevy's core architecture pattern
- **Clean Architecture**: Layered design with clear separation of concerns
- **Domain-Driven Design (DDD)**: Business logic organized by domain concepts
- **Event-Driven Architecture**: Communication via Bevy events and IPC

---

## Architecture

### Workspace Structure
The project is organized as a Cargo workspace with two main Rust crates and a React application:

```
lifthrasir/
├── game-engine/        # Core game engine (Bevy ECS)
├── src-tauri/          # Tauri integration layer
└── web-ui/             # React frontend UI
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
# Run Tauri app (includes UI + game engine)
cd src-tauri
cargo tauri dev

# Or from root
cargo run
```

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
