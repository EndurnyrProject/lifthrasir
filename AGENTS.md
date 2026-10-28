# AGENTS.md - Lifthrasir Codebase Documentation

## Skills to use

- Load the bevy skill for any Bevy work (ECS, rendering, UI, assets, shaders) — it is the current 0.20 reference
- When working with UI, also load the bevy-feathers-bsn skill (BSN scenes, Feathers widgets, theming)
- When touching third-party crate wiring, load the lifthrasir-bevy-plugins skill
- Load the bevy-cheatbook skill only to translate stale cheatbook-era snippets; its API syntax is 0.18-era
- Always Load the ponytail skill, it helps you to be more token efficient

---

## Project Overview

**Lifthrasir** is a Ragnarok Online client implementation written in Rust using the Bevy game engine with a native Bevy UI. The project aims to recreate the classic MMORPG client while leveraging modern technologies for cross-platform compatibility, performance, and maintainability.

### Key Features
- Support for Ragnarok Online file formats (GRF, GND, GAT, RSW, RSM/RSM2, SPR, ACT).
  **GND/RSW/RSM/RSM2 and GR2 import are offline-only**; runtime keeps embedded
  GAT decoding, SPR/ACT sprites, STR effects, palettes and glTF. See the map
  runtime boundary below.
- 3D terrain rendering from converted glTF, with proper coordinate system translation
- Character rendering with equipment and animation systems
- Authentication and character management
- Native UI built with Bevy

---

## Technology Stack

### Core Technologies
- **Rust**: Primary programming language for game engine
- **Bevy 0.20.0**: ECS-based game engine for rendering, game logic, and UI (UI authored with BSN + `bevy_feathers`)

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
├── apps/
│   ├── lifthrasir/              # Client binary and explicit plugin composition
│   ├── grf-utils/               # GRF inspection, pak/merge CLI
│   └── ro-to-lifthrasir-cli/     # Conversion dispatcher and protobuf generation
├── crates/
│   ├── lifthrasir-data/         # Bevy-free LIF/catalog/effect data
│   ├── lifthrasir-pak/          # Bevy-free archive reader/writer
│   ├── lifthrasir-assets/       # ro:// source, loaders, catalogs, processing
│   ├── lifthrasir-runtime/      # Content actors/effects/audio/world capabilities
│   ├── lifthrasir-ui/           # Generic Feathers/chrome/text/meter primitives
│   ├── lifthrasir-tooling/      # Bevy-free offline import/conversion/pack inputs
│   ├── lifthrasir-client/       # Client state, settings, gameplay and adapters
│   ├── lifthrasir-client-ui/    # Login/HUD/screens and gameplay UI
│   ├── net-contract/           # Protocol-neutral Bevy Messages
│   ├── net-aesir/              # QUIC transport and protobuf codec
│   └── ro-formats/             # RO readers; GR2 import is offline-only
├── assets/                     # Root-CWD bootstrap and content
└── icons/                      # Packaging icons
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

`lifthrasir-client` and `lifthrasir-client-ui` use `net-contract`, never a
transport/codec. `crates/lifthrasir-client/tests/workspace_boundaries.rs` checks
normal/build package identities; `no_transport_dep.rs` also checks the isolated
client tree. Shared assets/runtime/UI have no client or network dependency.

The adapter is wired at `apps/lifthrasir/src/main.rs`, which adds `AesirNetPlugin`
when the default `net-aesir` feature is enabled. `--no-default-features` compiles
without networking; that alone does not produce a playable client.

**To support a different protocol (e.g. rAthena):** implement a new crate that
depends only on `net-contract`, write the inbound event Messages from incoming
packets, read the outbound command Messages and translate them to outgoing packets,
expose a plugin, and add it at the binary. The contract, client and UI libraries
stay untouched.


### Map runtime boundary

Maps are **converted offline to glTF**, not loaded from standalone RO map files.
GND/RSW/RSM/RSM2 parsing is offline-only; embedded GAT decoding remains runtime.
A map is a single
`data/maps/<map>/<map>.glb` produced by `ro-to-lifthrasir-cli convert-maps`,
carrying chunked indexed terrain meshes, baked `KHR_lights_punctual` lighting
(sun, point and spot) and ambient, sound and effect emitters, prop references,
water parameters **plus its baked water tile mask**, and the original `.gat`
bytes verbatim in the binary chunk. Ground textures are shared across maps in
the `ground/tex/` pool and referenced by absolute `ro://ground/tex/<name>.ktx2`
URI; prop textures stay in `models/tex/`. Both pools write **BC7 KTX2**, so a
headless host that loads one needs `CompressedImageFormats::BC` (the windowed
`RenderPlugin` sets it from the device features).

Consequences worth knowing before changing map code:

- **A missing or invalid map glb is a hard failure**, not a fallback. Shared
  `MapSceneRequest`/`MapSceneStatus` own loading/dependency readiness; the client
  selects the active root and turns a selected scene failure into a path-qualified
  fatal error. No raw-map fallback exists.
- **`ro-formats`' GAT parser is still a runtime dependency.** `LIF_gat` stores
  raw `.gat` bytes that the runtime decodes into `CurrentMapPathfindingGrid`
  and `CurrentMapAltitude`. Only the GND/RSW/RSM/RSM2 parsers are offline-only.
  Do not "move all format parsing offline" — it will break walkability,
  grounding and terrain raycast.
- **Water tile selection happens in the converter, not the runtime.** The
  runtime has no GND, and selection runs on GND heights. GAT heights usually
  follow the GND surface but retail hand-edits some cells, so they are not a
  substitute and selection cannot be recomputed at load. The converter bakes
  a tile bitmask into `LIF_water`.
- **GAT heights follow the terrain in retail data.** Each GND cell covers
  2 x 2 GAT cells; a GAT corner height is the bilinear interpolation of the
  GND cell's four corner heights at that corner (native units, same corner
  order, no offset). This holds for 95-99.8% of corners on converted retail
  maps; the rest are hand-edited. Anything that reshapes terrain should
  rewrite the GAT heights under it with this rule.
- **Props resolve only to `.glb`.** Conversion fails loudly on a prop it could
  not convert, so a written map glb is loadable by construction.
- Adding a map feature means extending the **converter schema**
  (`crates/lifthrasir-data/src/lif.rs`) and the glb writer, not adding a runtime
  parser. Bump `lif::MAP_FORMAT_VERSION` for map schema changes and
  `lif::MODEL_FORMAT_VERSION` for prop model changes; the two are independent,
  so a map bump no longer invalidates cached models. `LIF_map.source` (the
  retail RSW/GND/GAT hashes) is optional: converted maps carry it, editor-
  authored maps do not.

### Shared host composition

Register `ro://` with `register_ro_asset_source` **before** `AssetPlugin` (or
`DefaultPlugins`). After renderer/asset stores exist, add `ContentAssetsPlugin`,
`ContentCatalogPlugin`, then the runtime capabilities you need or
`ContentRuntimePlugins`. The full group owns Kira/spatial audio; disable
`ContentAudioPlugin` for a device-free host and disable Bevy's built-in audio when
using Kira. Actors/effects need `RuntimeCorePlugin`; world GLB loading requires
Bevy glTF/scene/mesh/animation support. Use the examples as executable composition
references, not `ClientPlugins` as a shared prerequisite.

Choose the Feathers theme before `UiPrimitivesPlugin`, which owns generic focus
and widgets. `ClientUiPlugin` owns gameplay focus/cursor/HDR policy. The binary
alone composes contract, content, client, adapter and UI groups, each once.

### Building

```bash
# Build client library only, from the workspace root
cargo build -p lifthrasir-client

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

This writes `assets/data/maps/<map>/<map>.glb`, pools its ground textures under
`assets/data/ground/tex/` (`--ground-dir`, default `assets/data/ground`), and
converts the props they reference into `assets/data/models`. It removes any
stale `maps/<map>/tex/` directory left by older converters, processes maps in
sorted order, logs and continues past failures, and exits non-zero if any map
failed; writes are atomic so a failed map leaves no partial glb. `--force-models`
re-converts props that already exist on disk — required after a
`lif::MODEL_FORMAT_VERSION` bump, since cached props are otherwise skipped and
the runtime rejects stale ones. Single map: `convert-map --map <name>`.

Convert standalone props before packing too: **`pack` only archives files
already on disk; it never converts them.** The default set is the ten Hunter
trap props:

```bash
cargo run --release -p ro-to-lifthrasir-cli -- convert-props
```

These become `assets/data/models/외부소품/트랩*.glb`, which skill ids 115–125
resolve as armed traps at runtime. A pak built without them leaves Hunter traps
invisible.

Convert GR2 actors before packing too (guild flags, guardians, Emperium and
other available GR2 models):

```bash
cargo run --release -p ro-to-lifthrasir-cli -- convert-gr2
```

This writes self-contained `assets/data/models/3dmob/*.glb` files with their
embedded textures and available idle/walk/attack/hit/death clips. It uses
`assets/convert.toml` and processes the effective GRF overlay in sorted order,
reports individual failures, and exits nonzero if any model failed. Single model:
`convert-gr2 --model guildflag90_1.gr2`. Writes replace existing outputs atomically
only after skin/animation validation.

The runtime routes `.gr2` actor resource names to these GLBs, never to SPR/ACT.
The `ro-formats` `gr2` feature belongs to offline tooling only; the client uses
Bevy glTF skinning/animation and the shared guild-emblem cache. Missing optional
action clips use idle; missing or invalid model GLBs fail loudly. No map format
version bump is needed. Validate local converted assets without opening a window:

```bash
cargo test -p lifthrasir-client --test gr2_assets -- --ignored
```

Then produce the pak from retail GRFs (GRF parsing lives only in the offline
tooling):

```bash
cargo run --release -p grf-utils -- pack \
  --grf assets/en.grf --grf assets/data.grf \
  --content-dir assets/data \
  --out assets/lifthrasir.pak
```

`pack` **excludes `.rsm`, `.rsm2`, `.gnd`, `.gat` and `.rsw`** from the archive:
nothing reads them at runtime, and each map glb already embeds its `.gat` bytes.
The filter is applied after the precedence tiers are merged, so no tier can
reintroduce them; the summary reports an `Excluded:` count. The manifest
`content_version` defaults to the pack timestamp (Unix milliseconds), and
`merge` only requires a patch to be newer than the main pak, so ordering is
automatic. `--content-version` is an override for reproducing an earlier build.

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
cargo test -p lifthrasir-client --test effect_assets -- --ignored
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
- **NVIDIA DLSS Super Resolution SDK v310.9.1** (required by `dlss_wgpu` 6.0, which Bevy 0.20 uses; other SDK versions fail to compile) — clone [NVIDIA/DLSS at `v310.9.1`](https://github.com/NVIDIA/DLSS/tree/v310.9.1) (it is not redistributable) and point `DLSS_SDK` at its absolute path. Re-check the `dlss_wgpu` version chart on every Bevy upgrade.
- **Vulkan SDK** — with `VULKAN_SDK` set.
- **Clang** — required by `bindgen` when building the SDK wrapper.

At runtime DLSS degrades gracefully: if the GPU/driver does not support it, the `DlssSuperResolutionSupported` resource is absent and the setting stays `Off` (logged once). The setting lives in the Graphics menu as `Off / DLAA / Quality / Balanced / Performance / Ultra Performance` and is orthogonal to the xBRZ "Upscaling" setting (DLSS scales render resolution; xBRZ bakes textures).

**Licensing / distribution (settle before any public release):**
- The DLSS SDK license text (DLSS Programming Guide §9.5) must ship alongside any distributed binary.
- The proprietary DLSS runtime library must be packaged next to the binary: `nvngx_dlss.dll` (Windows) or `libnvidia-ngx-dlss.so.310.9.1` (Linux), from `$DLSS_SDK/lib/<platform>/rel/`.
- The binary is already GPL-3.0 (via the xBRZ `xbrz-rs` crate); GPL plus the proprietary DLSS blob loaded at runtime is a known gray area — resolve it before distributing publicly.

Manual verification checklist: `specs/2026-06-28-dlss/design.md` → "Testing".

### Generating network protobuf types

The client talks to the aesir account server over QUIC using protobuf (`bevy_quinnet` + `prost`). The Rust types are generated from aesir's canonical `aesir.proto` and committed at `crates/net-aesir/src/proto/aesir.net.rs`. Re-run this whenever that schema changes:

```bash
cargo run -p ro-to-lifthrasir-cli -- gen-proto \
  --src <aesir>/apps/commons/proto \
  --out crates/net-aesir/src/proto/aesir.net.rs
```

This uses the pure-Rust `protox` compiler, so no system `protoc` is required. Commit the regenerated file.

### Testing

```bash
# Run all tests
cargo test

# Focused standalone and client-linked boundaries
cargo test --locked -p lifthrasir-client --test workspace_boundaries --test shared_plugin_isolation
cargo test --locked -p lifthrasir-client --test settings_compatibility
cargo test --locked -p lifthrasir-runtime --test plugin_composition
cargo check --locked -p lifthrasir-runtime --examples
```

The examples compile without a client session. Running `effect_preview` or
`content_viewer` opens a window (the latter also opens audio), so obtain approval
and suitable hardware first; see README commands. CPU tests do not validate
rendering, sound, login, equipment previews or map-warp gameplay.

The two ignored `effect_assets`/`gr2_assets` commands above are read-only checks
against locally provisioned content, not fixture generation. Tooling's ignored
`regenerate_game_engine_fixtures` and `generate_mip_probe_fixture` tests **write**
`crates/lifthrasir-runtime/tests/fixtures`; do not run them for validation. Keep
historical design notes as historical evidence rather than rewriting old paths.

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
7. The bevy cheatbook (https://bevy-cheatbook.github.io/) is unmaintained and ~0.14-era: trust its concepts, not its API syntax, and prefer the bevy skill or docs.rs for names.
8. Consult bevy examples, they are very helpful https://github.com/bevyengine/bevy/tree/latest/examples#examples
9. Also check the bevy documentation for up-to-date function https://docs.rs/bevy/latest/bevy/
