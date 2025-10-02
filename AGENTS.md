# AGENTS.md - Lifthrasir Codebase Documentation

## Table of Contents
1. [Project Overview](#project-overview)
2. [Technology Stack](#technology-stack)
3. [Architecture](#architecture)
4. [Core Libraries](#core-libraries)
5. [How to Use bevy_auto_plugin](#how-to-use-bevy_auto_plugin)
6. [Project Structure](#project-structure)
7. [Key Systems](#key-systems)
8. [Integrations](#integrations)
9. [Development Workflow](#development-workflow)
10. [Known Technical Debt](#known-technical-debt)

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

## Core Libraries

### Entity Hierarchy & State Management

**Entity Hierarchies**
- `moonshine-object = "0.2.6"`
  - Ergonomic interface for complex entity hierarchies
  - Used for character sprite layers (body, equipment, effects)
  - Provides object-based queries

- `moonshine-kind = "0.3"`
  - Adds type information to entities
  - `Instance<T>` type for typed entity references
  - Prevents errors from mixing different entity types

- `moonshine-tag = "0.3.0"`
  - Fast, unique identifiers for entities

**State Machines**
- `seldom_state = "0.14.0"`
  - State machine implementation for Bevy
  - Used for character animation states
  - Supports complex state transitions with triggers

### Frontend (React UI)

**Core**
- `react = "18.3.1"`
- `react-dom = "18.3.1"`
- `@tauri-apps/api = "^2"`
  - Tauri API for React
  - IPC communication with backend

**Development**
- `typescript = "~5.6.2"`
- `vite = "^6.0.3"`
- `@vitejs/plugin-react = "^4.3.4"`
- `@types/react = "^18.3.1"`
- `@types/react-dom = "^18.3.1"`
---

## How to Use bevy_auto_plugin

### Overview

`bevy_auto_plugin` is a critical library in Lifthrasir that eliminates boilerplate code in Bevy plugins. Instead of manually registering components, events, systems, and resources in the `Plugin::build()` method, bevy_auto_plugin uses attribute macros to automatically handle registration at compile time.

**Why We Use It:**
- **Reduces Boilerplate**: No more writing repetitive `.add_systems()`, `.register_type()`, `.init_resource()` calls
- **Prevents Errors**: Can't forget to register a component or event
- **Cleaner Code**: Keep type definitions and registration logic together
- **Better Organization**: Each type is self-contained with its registration metadata

**Version**: `bevy_auto_plugin = "0.5.0"`

**Compatibility**: Bevy 0.17 (we're on 0.16.1, but it's compatible)

---

### Creating Auto Plugins

#### Basic Plugin Declaration

```rust
use bevy_auto_plugin::prelude::*;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct MyDomainPlugin;
```

**Attributes Explained:**
- `#[derive(AutoPlugin)]`: Derives the auto-plugin functionality
- `#[auto_plugin(impl_plugin_trait)]`: Automatically implements `Plugin` trait for you

This generates the `Plugin` implementation with all registered types automatically added to `App` in the `build()` method.

---

### Auto-Registering Components

Components are automatically registered with reflection support:

```rust
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[auto_component(
    plugin = MyDomainPlugin,
    derive(Debug, Default, Clone),
    reflect(Debug, Default),
    register,
    auto_name,
)]
pub struct CharacterData {
    pub id: u32,
    pub name: String,
    pub level: u8,
}
```

**Attributes Explained:**
- `plugin = MyDomainPlugin`: Associates this component with MyDomainPlugin
- `derive(...)`: Standard Rust derives
- `reflect(...)`: Traits to register with Bevy's reflection system
- `register`: Registers the type with Bevy's type registry (for reflection)
- `auto_name`: Automatically sets the type name for reflection

**What This Does:**
1. Adds `Component` derive
2. Adds `Reflect` derive
3. Automatically calls `app.register_type::<CharacterData>()` in plugin
4. Registers reflection traits (Debug, Default)

---

### Auto-Registering Events

Events work similarly to components:

```rust
#[auto_event(
    plugin = MyDomainPlugin,
    derive(Debug, Clone),
    reflect(Debug),
    register,
    auto_name,
)]
pub struct CharacterSpawned {
    pub character_id: u32,
    pub position: Vec3,
}
```

**What This Does:**
1. Adds `Event` derive
2. Adds `Reflect` derive
3. Automatically calls `app.add_event::<CharacterSpawned>()` in plugin
4. Registers type with reflection system

---

### Auto-Registering Systems

Systems are registered to run in specific schedules with configuration:

```rust
#[auto_system(
    plugin = MyDomainPlugin,
    schedule = Update,
)]
pub fn update_character_positions(
    mut query: Query<(&mut Transform, &Velocity), With<CharacterData>>,
    time: Res<Time>,
) {
    for (mut transform, velocity) in &mut query {
        transform.translation += velocity.0 * time.delta_secs();
    }
}
```

**Attributes Explained:**
- `plugin = MyDomainPlugin`: Associates system with plugin
- `schedule = Update`: Runs in `Update` schedule (can be `Startup`, `PostUpdate`, etc.)

**Advanced System Configuration:**

```rust
#[auto_system(
    plugin = MyDomainPlugin,
    schedule = Update,
    config(
        run_if = in_state(GameState::Playing),
        after = physics_system,
        before = render_system,
    ),
)]
pub fn complex_system(/* ... */) {
    // System logic
}
```

**Configuration Options:**
- `run_if = <condition>`: System only runs if condition is true
- `after = <system>`: Runs after specified system
- `before = <system>`: Runs before specified system
- `in_set = <system_set>`: Adds to a system set

---

### Auto-Registering Resources

Resources can be automatically initialized:

```rust
#[auto_resource(
    plugin = MyDomainPlugin,
    derive(Debug, Default),
    reflect(Debug),
    register,
    auto_name,
)]
pub struct GameSettings {
    pub music_volume: f32,
    pub sfx_volume: f32,
}
```

**What This Does:**
1. Adds `Resource` derive
2. Adds `Reflect` derive (if specified)
3. Automatically calls `app.init_resource::<GameSettings>()` in plugin
4. Requires `Default` implementation for initialization

**For Resources Without Default:**

If your resource doesn't implement `Default`, you can insert it manually in the plugin's `build()` method while still using auto-registration for reflection:

```rust
#[auto_resource(
    plugin = MyDomainPlugin,
    reflect(Debug),
    register,
    auto_name,
    // Note: no init attribute, so won't call init_resource
)]
pub struct ConnectionPool {
    pool: Vec<Connection>,
}

// Then in plugin implementation:
impl Plugin for MyDomainPlugin {
    fn build(&self, app: &mut App) {
        // Auto-registered types are added here automatically

        // Manually insert resource with custom initialization
        app.insert_resource(ConnectionPool {
            pool: create_connection_pool(),
        });
    }
}
```

---

### Working with Generic Types

bevy_auto_plugin supports generic types with concrete type specification:

```rust
#[auto_component(
    plugin = MyDomainPlugin,
    generics(String),  // Specify concrete type for generic
    derive(Debug, Clone),
    register,
    auto_name,
)]
pub struct Container<T> {
    pub value: T,
}
```

**Multiple Generic Parameters:**

```rust
#[auto_component(
    plugin = MyDomainPlugin,
    generics(String, u32),  // First is T, second is U
    derive(Debug),
    register,
    auto_name,
)]
pub struct GenericPair<T, U> {
    pub first: T,
    pub second: U,
}
```

This registers `Container<String>` and `GenericPair<String, u32>` specifically.

---

### Quick Reference

| Task | Attribute | Required Fields |
|------|-----------|----------------|
| Create Plugin | `#[derive(AutoPlugin)]` + `#[auto_plugin(impl_plugin_trait)]` | None |
| Register Component | `#[auto_component(...)]` | `plugin` |
| Register Event | `#[auto_event(...)]` | `plugin` |
| Register Resource | `#[auto_resource(...)]` | `plugin` |
| Register System | `#[auto_system(...)]` | `plugin`, `schedule` |

**Common Attributes:**
- `plugin = <PluginName>` - Required on all
- `derive(...)` - Standard Rust derives
- `reflect(...)` - Reflection trait registration
- `register` - Enable type registration
- `auto_name` - Auto-set type name
- `schedule = <Schedule>` - System schedule (Update, Startup, etc.)
- `config(...)` - System configuration (run_if, before, after, in_set)
- `generics(...)` - Concrete types for generics

---

### Further Reading

- [bevy_auto_plugin GitHub](https://github.com/StrikeForceZero/bevy_auto_plugin)
- [Bevy Plugin Documentation](https://docs.rs/bevy/latest/bevy/app/trait.Plugin.html)
- [Bevy Reflection Guide](https://bevyengine.org/learn/book/plugin-development/)

## Key Systems

### 1. Hierarchical Asset System

**Purpose**: Unified asset loading from multiple sources (GRF archives, data folders, embedded assets)

**Implementation**:
- Custom `ro://` asset protocol registered with Bevy
- `RoAssetsPlugin` sets up the asset source before `AssetPlugin`
- `CompositeAssetSource` combines multiple sources with priority:
  1. Data folder (highest priority)
  2. GRF archives (in configured order)
  3. Embedded assets (fallback)

**Key Files**:
- `game-engine/src/infrastructure/assets/ro_assets_plugin.rs`
- `game-engine/src/infrastructure/assets/sources/composite.rs`
- `game-engine/src/infrastructure/assets/hierarchical_reader.rs`

**Usage**:
```rust
// Load any RO asset using ro:// protocol
let sprite = asset_server.load("ro://data\\sprite\\인간족\\몸통\\여\\여_body.spr");
let act = asset_server.load("ro://data\\sprite\\인간족\\몸통\\여\\여_body.act");
```

**Configuration**: `assets/loader.data.toml`
```toml
[assets]
grf = [
    { path = "data.grf", priority = 2 },
    { path = "en.grf", priority = 1 }
]
data_folder = "assets/data"
```

### 2. Generic Sprite Rendering System

**Purpose**: Reusable sprite animation system for any RO sprite (characters, items, effects)

**Core Components**:
- `RoAnimationController`: Controls sprite animation state
  - Current action index
  - Frame tracking
  - Animation delays
  - Optional palette for color variations
  - Looping control

- `RoSpriteFactory`: Factory for spawning sprites
  - `spawn_from_handles()`: Immediate spawn with loaded assets
  - `spawn_from_paths()`: Async spawn with asset loading
  - `spawn_hair_preview()`: Convenience for hair previews
  - `spawn_equipment_preview()`: Convenience for equipment

**Animation System**: `animate_sprites` system
- Advances frames based on ACT timing data
- Applies custom palettes (hair colors, etc.)
- Converts SPR frames to Bevy images with transparency

**Key Files**:
- `game-engine/src/domain/entities/components.rs`
- `game-engine/src/domain/entities/sprite_factory.rs`
- `game-engine/src/domain/entities/animation.rs`

**Usage Example**:
```rust
// Character customization preview
RoSpriteFactory::spawn_hair_preview(
    &mut commands,
    hair_sprite_handle,
    hair_act_handle,
    Some(hair_color_palette_handle),
    position,
);
```

### 3. Unified Character Entity System

**Purpose**: Complete character representation with state machines and sprite hierarchies

**Architecture**:
- Uses `moonshine-object` for complex entity hierarchies
- Uses `seldom_state` for character state machines
- Three-tier state system:
  - `AnimationState`: Visual state (Idle, Walking, Attacking, etc.)
  - `GameplayState`: Game logic state (Normal, Dead, Mounted, etc.)
  - `ContextState`: Screen context (CharacterSelection, InGame, etc.)

**Components**:
- `CharacterData`: ID, name, stats
- `CharacterAppearance`: Job, gender, hair style/color
- `EquipmentSet`: All equipped items by slot
- `CharacterSprite`: Visual representation

**Sprite Hierarchy**:
```
CharacterRoot
├── Body
├── Equipment/HeadBottom
├── Equipment/HeadTop
├── Equipment/HeadMid
├── Equipment/Weapon
├── Equipment/Shield
├── Effects/Aura
└── Shadow
```

**Key Files**:
- `game-engine/src/domain/entities/character/mod.rs`
- `game-engine/src/domain/entities/character/sprite_hierarchy.rs`
- `game-engine/src/domain/entities/character/states.rs`
- `game-engine/src/domain/entities/character/components/`

### 4. Terrain Generation

**Purpose**: Convert RO ground files (GND) to 3D Bevy meshes

**Process**:
1. Load GND file (terrain heightmap, textures, lighting)
2. Generate mesh vertices from height data
3. Calculate normals using cross product of diagonals
4. Smooth normals by averaging neighbors
5. Apply textures and lighting
6. Create Bevy mesh and material

**Coordinate Translation**: RO → Bevy
- RO: Top-left origin, Y=up, Z=south
- Bevy: Center origin, Y=up, Z=forward
- Cell size: 10.0 (2x RO's 5.0 for scale)
- Height multiplier: 5.0

**Key Files**:
- `game-engine/src/domain/world/terrain.rs`
- `game-engine/src/utils/coordinates.rs`
- `game-engine/src/infrastructure/ro_formats/gnd.rs`

### 5. Camera System

**Purpose**: RO-style camera with full control

**Features**:
- Isometric-like view angle
- WASD/Arrow keys: Move horizontally
- Q/E: Move up/down
- Mouse wheel: Zoom
- Left mouse drag: Pan
- Right mouse drag: Rotate
- R: Reset to initial position

**Implementation**:
- `Vec3::NEG_Y` as up vector (inverted for RO style)
- Camera positioned high above terrain
- Looks at map center

**Key Files**:
- `game-engine/src/domain/camera/controller.rs`
- `game-engine/src/domain/camera/components.rs`

## Development Workflow

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

### Asset Configuration

Edit `assets/loader.data.toml`:
```toml
[assets]
grf = [
    { path = "path/to/data.grf", priority = 2 },
    { path = "path/to/en.grf", priority = 1 }
]
data_folder = "assets/data"
```

---

## Best Practices for Contributors

### When Adding New Features

1. **Check existing patterns**: Look for similar features before implementing
2. **Use bevy_auto_plugin**: Don't manually register systems/events
3. **Follow layer separation**: Domain logic separate from infrastructure
4. **Write tests**: Add tests for new domain logic
5. **Update documentation**: Keep CLAUDE.md and this file current

---

**Last Updated**: 2025-10-03
**Project Version**: 0.1.0
**Bevy Version**: 0.16.1
**Tauri Version**: 2.0
