# AGENTS.md - Lifthrasir Codebase Documentation

## Skills to use

- Load the bevy-cheatbook skill
- When working with UI, load the bevy-enhanced-ui skill
- Always Load the ponytail skill, it helps you to be more token efficient

---

## Project Overview

**Lifthrasir** is a Ragnarok Online client implementation written in Rust using the Bevy game engine with a native Bevy UI. The project aims to recreate the classic MMORPG client while leveraging modern technologies for cross-platform compatibility, performance, and maintainability.

### Key Features
- Support for Ragnarok Online file formats (GRF, GND, GAT, RSW, RSM/RSM2, SPR, ACT).
  **Map and model formats are read by the offline tooling only** — see "Map
  runtime boundary" below. At runtime the client reads SPR/ACT sprites, STR
  effects, palettes, and glTF.
- 3D terrain rendering from converted glTF, with proper coordinate system translation
- Character rendering with equipment and animation systems
- Authentication and character management
- Native UI built with Bevy

---

## Technology Stack

### Core Technologies
- **Rust**: Primary programming language for game engine
- **Bevy 0.19.0**: ECS-based game engine for rendering, game logic, and UI

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
`lifthrasir-ui` stay untouched.


### Map runtime boundary

Maps are **converted offline to glTF and never parsed at runtime.** The client
contains no GND, GAT, RSW, RSM or RSM2 reader; a map is a single
`data/maps/<map>/<map>.glb` produced by `ro-to-lifthrasir-cli convert-maps`,
carrying terrain meshes, baked `KHR_lights_punctual` lighting and ambient,
sound and effect emitters, prop references, water parameters **plus its baked
water tile mask**, and the original `.gat` bytes verbatim in the binary chunk.

Consequences worth knowing before changing map code:

- **A missing or invalid map glb is a hard failure**, not a fallback. There is
  no second loading path to degrade into. `spawn_gltf_map` is the sole claimer
  of a map request and `detect_gltf_map_load_failure` panics naming the
  expected path.
- **`ro-formats`' GAT parser is still a runtime dependency.** `LIF_gat` stores
  raw `.gat` bytes that the runtime decodes into `CurrentMapPathfindingGrid`
  and `CurrentMapAltitude`. Only the GND/RSW/RSM/RSM2 parsers are offline-only.
  Do not "move all format parsing offline" — it will break walkability,
  grounding and terrain raycast.
- **Water tile selection happens in the converter, not the runtime.** The
  runtime has no GND, and GND and GAT heights are independent fields with no
  transform between them, so selection cannot be recomputed at load. The
  converter bakes a tile bitmask into `LIF_water`.
- **Props resolve only to `.glb`.** Conversion fails loudly on a prop it could
  not convert, so a written map glb is loadable by construction.
- Adding a map feature means extending the **converter schema**
  (`lifthrasir-data/src/lif.rs`) and the glb writer, not adding a runtime
  parser. Bump `lif::FORMAT_VERSION` when the schema changes — note it is
  shared by `LifMap` and `LifModel`, so bumping it invalidates converted
  **models** as well as maps.

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

### Game assets (Lifthrasir pak)

The runtime does **not** read GRF archives. All game content loads from a
"pak" — a plain zip64 archive with normalized paths (forward-slash, lowercase,
UTF-8), per-entry zstd/Stored compression, and a `.lifthrasir/manifest.toml`
entry (`format_version`, `content_version`). `assets/loader.toml` lists the
archives (`[[assets.archive]]`, resolved against `assets/`) plus the loose
`data_folder` override, which always wins for development.

That `data_folder` (`assets/data`) is the **root of the `ro://` namespace**, and
everything the client loads at runtime goes through it — retail content under
`ro://data/...` (from the GRFs) and Lifthrasir's own content beside it:
`ro://fonts/…`, `ro://shaders/…`, `ro://ui/icons/…`, `ro://config/clientinfo.toml`,
`ro://ron/…`, `ro://effects/…`, `ro://textures/…`. Nothing loads from Bevy's
default `assets/` source. The only files read straight off disk are
`assets/loader.toml` (the bootstrap config, read before the source exists) and
the hotbar save file.

Convert the maps first — **`pack` never converts anything**, it only archives
what is already on disk, and the runtime cannot load an unconverted map:

```bash
cargo run --release -p ro-to-lifthrasir-cli -- convert-maps --force-models
```

This writes `assets/data/maps/<map>/<map>.glb` plus `tex/*.png`, and the props
they reference into `assets/data/models`. It processes maps in sorted order,
logs and continues past failures, and exits non-zero if any map failed; writes
are atomic so a failed map leaves no partial glb. `--force-models` re-converts
props that already exist on disk — required after any `lif::FORMAT_VERSION`
bump, since cached props are otherwise skipped and the runtime rejects stale
ones. Single map: `convert-map --map <name>`.

Then produce the pak from retail GRFs (GRF parsing lives only in the offline
tooling):

```bash
cargo run --release -p grf-utils -- pack \
  --grf assets/en.grf --grf assets/data.grf \
  --content-dir assets/data \
  --out assets/lifthrasir.pak --content-version 1
```

`pack` **excludes `.rsm`, `.rsm2`, `.gnd`, `.gat` and `.rsw`** from the archive:
nothing reads them at runtime, and each map glb already embeds its `.gat` bytes.
The filter is applied after the precedence tiers are merged, so no tier can
reintroduce them; the summary reports an `Excluded:` count. `--content-version`
is enforced monotonic, so bump it on every rebuild.

Earlier `--grf` flags win on duplicate paths. `--content-dir` (repeatable) packs
a folder **at the pak root**, mirroring the runtime's `data_folder`, and is what
ships the client's own fonts/shaders/ui/ron content; `--data-folder` (optional)
packs a retail loose `data` folder **under its own name**, so it shadows the
GRFs' `data/...` entries. Both win over all GRFs.

Apply a patch pak (game must be closed; the patch file is consumed, the main pak
is atomically replaced and compacted):

```bash
cargo run --release -p grf-utils -- merge \
  --main assets/lifthrasir.pak --patch patch.pak
```

A missing or invalid pak fails startup loudly by design. Any zip tool can
inspect a pak. Validate all effect-catalog asset references against a built pak
with:

```bash
cargo test -p game-engine --test effect_assets -- --ignored
```

`ro-to-lifthrasir-cli` still reads raw GRFs for offline conversion via its own
`assets/convert.toml`.

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
