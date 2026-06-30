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
├── net-contract/       # Protocol-neutral network contract (Bevy Messages)
├── net-aesir/          # Aesir QUIC network adapter (transport + codec)
└── grf-utils/          # GRF archive utilities
```

### Network boundary

The network stack is split into a protocol-neutral contract and swappable adapters:

- **`net-contract`** is the protocol-neutral Bevy `Message` contract — inbound
  server→client `events`, outbound client→server `commands`, and the neutral
  `dto`/`state` types they reference. It depends only on `bevy`; it knows nothing
  about any wire protocol.
- **Adapter crates** (e.g. `net-aesir`, the aesir QUIC adapter) own the transport
  and codec (`bevy_quinnet` + `prost`). An adapter reads the outbound command
  Messages and writes the inbound event Messages; that is its entire interface to
  the rest of the app.

`game-engine` and `lifthrasir-ui` depend **only** on `net-contract` and never on a
transport/codec. This is locked in by `game-engine/tests/no_transport_dep.rs`, which
fails if `game-engine`'s dependency tree regains `bevy_quinnet`, `prost`, or an
adapter crate.

The adapter is wired at the binary, not in `game-engine`: the `lifthrasir` binary
(`lifthrasir/src/main.rs`) depends on `net-aesir` and adds its `AesirNetPlugin`.

**To support a different protocol (e.g. rAthena):** implement a new crate that
depends only on `net-contract`, write the inbound event Messages from incoming
packets, read the outbound command Messages and translate them to outgoing packets,
expose a plugin, and add that plugin in `main.rs`. The contract, `game-engine`, and
`lifthrasir-ui` stay untouched. See
`specs/2026-06-30-network-decoupling/design.md`.


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

### DLSS Super Resolution (optional, NVIDIA / Windows / Linux)

DLSS is an **opt-in, off-by-default** Cargo feature. It is absent from default builds and **cannot compile on macOS** (it requires the Vulkan backend and an NVIDIA RTX GPU). Build and run it only on Windows or Linux with an RTX card:

```bash
DLSS_SDK=/path/to/dlss-sdk VULKAN_SDK=/path/to/vulkan \
  cargo run -p lifthrasir --features dlss
```

Prerequisites on the target machine:
- **NVIDIA DLSS Super Resolution SDK v310.5.3** — download separately (it is not redistributable) and point `DLSS_SDK` at its absolute path.
- **Vulkan SDK** — with `VULKAN_SDK` set.
- **Clang** — required by `bindgen` when building the SDK wrapper.

At runtime DLSS degrades gracefully: if the GPU/driver does not support it, the `DlssSuperResolutionSupported` resource is absent and the setting stays `Off` (logged once). The setting lives in the Graphics menu as `Off / DLAA / Quality / Balanced / Performance / Ultra Performance` and is orthogonal to the xBRZ "Upscaling" setting (DLSS scales render resolution; xBRZ bakes textures).

**Licensing / distribution (settle before any public release):**
- The DLSS SDK license text (DLSS Programming Guide §9.5) must ship alongside any distributed binary.
- The proprietary `nvngx_dlss` runtime libraries must be packaged next to the binary.
- The binary is already GPL-3.0 (via the xBRZ `xbrz-rs` crate); GPL plus the proprietary DLSS blob loaded at runtime is a known gray area — resolve it before distributing publicly.

Manual verification checklist: `specs/2026-06-28-dlss/design.md` → "Testing".

### Generating network protobuf types

The client talks to the aesir account server over QUIC using protobuf (`bevy_quinnet` + `prost`). The Rust types are generated from aesir's canonical `aesir.proto` and committed at `net-aesir/src/proto/aesir.net.rs`. Re-run this whenever that schema changes:

```bash
cargo run -p ro-to-lifthrasir-cli -- gen-proto \
  --src <aesir>/apps/commons/proto \
  --out net-aesir/src/proto/aesir.net.rs
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
