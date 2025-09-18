# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Lifthrasir is a Ragnarok Online client implementation written in Rust using the Bevy game engine. The project focuses on loading and rendering game assets from the original Ragnarok Online GRF (Game Resource File) format.

## Development Commands

### Build & Check
- `cargo build` - Build the project
- `cargo build --release` - Build optimized release version
- `cargo check` - Check code for compilation errors without building
- `cargo run` - Run the application

### Testing & Quality
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run a specific test
- `cargo clippy` - Run linter to catch common mistakes
- `cargo fmt` - Format code using rustfmt
- `cargo fmt --check` - Check formatting without applying changes

## Architecture

### Core Structure
The codebase follows Clean Architecture principles with Domain-Driven Design, organized into layers with clear separation of concerns:

#### **`src/domain/`** - Domain Layer (Core Business Logic)
- **`assets/`** - Asset management domain
  - Components for asset caching and management
  - Systems for asset lifecycle management
- **`camera/`** - Camera domain logic
  - Camera components and controller logic
  - Camera movement and control systems
- **`entities/`** - Game entity management
  - Animation systems and components
  - Entity lifecycle management
- **`world/`** - World and map domain
  - Terrain generation and management
  - Map loading and world state management

#### **`src/infrastructure/`** - Infrastructure Layer (External Concerns)
- **`ro_formats/`** - Ragnarok Online file format parsers
  - GRF, RSM, RSW, GND, GAT, ACT, SPR format parsing using `nom`
  - DES encryption support for legacy formats
- **`assets/`** - Asset loading infrastructure
  - Custom Bevy asset loaders for RO formats
  - Asset converters and BMP texture loading
- **`storage/`** - File system and storage
  - GRF file reading and caching mechanisms

#### **`src/presentation/`** - Presentation Layer (User Interface)
- **`input/`** - Input handling systems
  - Camera, keyboard, and mouse input processing
- **`rendering/`** - Rendering systems
  - Model rendering and material management
  - Lighting systems and terrain rendering
  - Water rendering with shaders

#### **`src/app/`** - Application Layer (Orchestration)
- Main application plugin and map loading plugin
- Coordinates between different layers

#### **`src/plugins/`** - Bevy Plugin Organization
- Individual plugins for assets, input, rendering, and world systems
- Clean separation of Bevy-specific concerns

#### **`src/core/`** - Shared Core Components
- Events, resources, and application state management
- Cross-cutting concerns shared across layers

#### **`src/utils/`** - Utility Functions
- Constants, coordinate transformations, and string utilities
- Helper functions used across the application

### Key Dependencies
- `bevy` - Game engine with dynamic linking enabled
- `nom` - Parser combinators for binary format parsing
- `flate2` - zlib decompression for GRF files
- `nalgebra` - Linear algebra for transformations
- `encoding_rs` - Korean text encoding support

## Ragnarok Online Format Overview

### File Formats
- **GRF (Game Resource File)**: Archive format containing all game assets, compressed with zlib
- **GND (Ground)**: Terrain mesh data with height maps, texture information, and lightmaps
- **GAT (Ground Altitude)**: Collision and walkability data for pathfinding
- **RSW (Resource World)**: World description with references to all objects, lights, sounds, and effects
- **RSM (Resource Model)**: 3D model format with hierarchical node structure for animated objects
- **ACT/SPR**: 2D sprite animations and sprite sheets

### Ragnarok Online World Structure
The RO world uses a cell-based system where:
- Each map is divided into a grid of cells
- GND defines the visual terrain (height, textures, normals)
- GAT defines walkability and collision for each cell
- RSW places all objects (models, lights, sounds, effects) in the world

## Coordinate System Translation

### Ragnarok Online Coordinate System
- **Origin**: Top-left corner of the map
- **X-axis**: Increases to the right (East)
- **Y-axis**: Height/elevation (up is positive)
- **Z-axis**: Increases downward (South)
- **Cell size**: Each terrain cell is 5x5 units in RO coordinates
- **Rotation order**: Z → X → Y (important for RSM model rotations)

### Bevy Coordinate System
- **Origin**: Center of the world
- **X-axis**: Right is positive
- **Y-axis**: Up is positive (standard 3D convention)
- **Z-axis**: Forward is negative (right-handed system)

### Our Translation Approach

#### Terrain Positioning (GND → Bevy)
```rust
// In domain/world/terrain.rs, we use CELL_SIZE = 10.0 (double RO's 5.0 for better visual scale)
let base_x = x as f32 * CELL_SIZE;
let base_z = y as f32 * CELL_SIZE;
// Heights are multiplied by 5.0 to match the horizontal scale
let height = surface.height[i] * 5.0;
```

#### Model Positioning (RSW → Bevy)
```rust
// From utils/coordinates.rs
let position = Vec3::new(
    model.position[0] + (map_width * 5.0),  // Center in X by adding half terrain width
    model.position[1],                       // Y (height) unchanged
    model.position[2] + (map_height * 5.0),  // Center in Z by adding half terrain height
);
```

#### Rotation Conversion
RO uses Euler angles in degrees with ZXY rotation order:
```rust
// Convert RO rotation to Bevy quaternion
let quat_z = Quat::from_rotation_z(model.rotation[2].to_radians());
let quat_x = Quat::from_rotation_x(model.rotation[0].to_radians());  
let quat_y = Quat::from_rotation_y(model.rotation[1].to_radians());
let rotation = quat_z * quat_x * quat_y;  // Apply in RO's order
```

### Camera System

#### Initial Camera Position
The camera is positioned to view the map from an isometric-like perspective:
```rust
// From domain/world/terrain.rs - setup_terrain_camera
let camera_pos = Vec3::new(
    map_center_x,           // Center horizontally
    -2000.0,                // High above the terrain
    -map_center_z * 2.5     // Back from center for better view
);
// Camera looks at map center with Y-down as up vector (inverted for RO style)
Transform::from_translation(camera_pos).looking_at(look_at, Vec3::NEG_Y)
```

#### Camera Controls
- **WASD/Arrow Keys**: Move camera in world space
- **Q/E**: Move camera up/down
- **Mouse Wheel**: Zoom in/out along view direction
- **Left Mouse Drag**: Pan camera
- **Right Mouse Drag**: Rotate camera view
- **R Key**: Reset camera to initial position

The camera uses `Vec3::NEG_Y` as the up vector to match Ragnarok Online's inverted Y-axis convention, creating the familiar RO viewing angle.

### Normal Calculation
Terrain normals are calculated using the cross product of quad diagonals (matching roBrowser's approach):
```rust
// SW to NE diagonal and SE to NW diagonal
let diag1 = northeast - southwest;
let diag2 = northwest - southeast;
let normal = diag1.cross(diag2).normalize();
```

Normals are then smoothed by averaging with neighboring cells for better lighting.

## Development Guidelines

1. **Always use Context7** to check libraries' available modules and functions before writing any code
2. **Consult the Bevy Cheatbook** for good practices and examples: https://bevy-cheatbook.github.io/
3. **Use Bevy code examples thoroughly** - find them at: https://github.com/bevyengine/bevy/tree/latest/examples#examples

### Best Practices
- Verify API availability before using any Bevy features or external crates
- Follow ECS patterns and conventions from the Bevy Cheatbook
- Reference official Bevy examples for implementation patterns
- Check Context7 documentation for up-to-date API usage

## Important Notes
- The project uses Bevy's dynamic linking feature for faster compilation during development
- Asset files in the `assets/` directory are gitignored and need to be provided separately
- The main window is configured with constants from `utils/constants.rs`
- Korean text encoding is handled via `encoding_rs` for proper string parsing from game files
- Terrain is generated at world origin (0, 0, 0) with models positioned relative to it
- The coordinate translation preserves RO's visual style while adapting to Bevy's coordinate conventions


## Colour Pallete

# UI Color Palette: Ashen Forged

This palette is designed for a clean, sharp, and modern UI with a dark theme. It uses a foundation of strong grays, allowing the vibrant "Runic Glow" to act as a clear and effective accent for all interactive elements.

## Main Palette

| Role      | Hex Code    | Name & Description                                                                            |
| :-------- | :---------- | :-------------------------------------------------------------------------------------------- |
| **Primary** | ` #1A1A1A ` | **Forge Soot:** A very dark, near-black charcoal. Forms the base of your UI.                    |
| **Secondary** | ` #2D3038 ` | **Slate Gray:** A dark gray for panels, windows, and surfaces that sit on the primary background. |
| **Tertiary** | ` #444444 ` | **Polished Steel:** A lighter gray for hover states, borders, and dividers.                   |
| **Accent** | ` #00E57A ` | **Energetic Green:** The bright, magical green from your logo. For all interactive elements. |
| **Highlight** | ` #E1E1E1 ` | **Ashen White:** A soft off-white for all primary body text and icons for readability.        |
| **SecondaryAccent** | ` #008080 ` | **Mystic:** Blueish Green for secondary accents.      |
| **Special** | ` #D4AF37 ` | **Gilded Accent:** The gold from the logo's text. Use sparingly for legendary items or titles. |

## Feedback Colors

These colors should be used to provide clear feedback to the player for common actions.

| State     | Hex Code    | Name & Description                                    |
| :-------- | :---------- | :---------------------------------------------------- |
| **Success** | ` #3E8A6B ` | **Muted Jade:** For positive confirmation and success messages. |
| **Warning** | ` #C7883C ` | **Amber:** For warnings or potentially risky actions.     |
| **Error** | ` #A44242 ` | **Worn Crimson:** For errors, failed actions, and alerts.  |

### Usage Notes

- **Contrast is key:** Ensure that the `Ashen White` text has sufficient contrast against both the `Forge Soot` and `Slate Gray` backgrounds for good readability.
- **Use accents intentionally:** The `Energetic Green` color should guide the user's eye to things they can click or interact with. Avoid using it for static text or non-interactive elements.
- **Keep it clean:** The strength of this palette is its simplicity. Avoid introducing many new colors to maintain a cohesive and professional look.
- Slight transparency can be applied to the grays for overlays or modals to add depth without introducing new colors.


## Libraries That we Use

- **bevy_auto_plugin**: Instead of registering systems, events, components and other bevy resources manually, always use bevy_auto_plugin, you can find examples in the 
codebase and also using context7.
- **moonshine-object**: An extension to Bevy which provides an ergonomic interface for managing complex Entity hierarchies. Use it when you need
to query complex hierarchies for the entities, like equipment on characters, or child entities on models.
- **moonshine-kind**: A problem with using entities in this way is the lack of information about the "kind" of the entity. This results in code that is error prone, hard to debug, and read.
This crate attempts to solve this problem by introducing a new Instance<T> type which behaves like an Entity but also contains information about the "kind" of the entity
- **moonshine-tag:** Cheap, fast, mostly unique identifiers designed for Bevy.
- **bevy_lunex**: Blazingly fast retained layout engine for Bevy entities, built around vanilla Bevy ECS. It gives you the ability to make your own custom UI using regular ECS like every other part of your app.
every UI should use bevy_lunex.



## Generic Sprite Rendering System

The project includes a generic, reusable sprite rendering system in `/src/domain/entities/` that can render any RO sprite with animation support. This system is independent of the character hierarchy and can be used for various purposes like UI sprites, item previews, effects, and character customization screens.

### Core Components

#### **`RoAnimationController`** (`/src/domain/entities/components.rs`)
The main component for controlling sprite animations:
- Supports custom palettes for hair colors and other variations
- Configurable action indices and looping behavior
- Tracks animation state (current frame, timer, delays)

#### **`RoSpriteFactory`** (`/src/domain/entities/sprite_factory.rs`)
Factory for spawning animated sprites with convenience methods:

**Spawn from pre-loaded handles** (immediate):
```rust
RoSpriteFactory::spawn_from_handles(
    &mut commands,
    sprite_handle,
    act_handle,
    Some(palette_handle), // Optional custom palette
    Vec3::new(0.0, 0.0, 0.0), // Position
    0, // Action index (0 = idle, 1 = walking, etc.)
)
```

**Spawn from file paths** (async loading):
```rust
RoSpriteFactory::spawn_from_paths(
    &mut commands,
    &asset_server,
    "data\\sprite\\인간족\\머리통\\여\\1_여.spr".to_string(),
    "data\\sprite\\인간족\\머리통\\여\\1_여.act".to_string(),
    Some("data\\palette\\머리\\1_여_5.pal".to_string()), // Hair color palette
    Vec3::new(0.0, 0.0, 0.0),
    0,
)
```

**Convenience methods**:
```rust
// For hair previews in character creation
RoSpriteFactory::spawn_hair_preview(
    &mut commands,
    head_sprite_handle,
    head_act_handle,
    Some(hair_color_palette_handle),
    position,
)

// For equipment previews
RoSpriteFactory::spawn_equipment_preview(
    &mut commands,
    equipment_sprite_handle,
    equipment_act_handle,
    position,
)
```

### Animation System

The `animate_sprites` system (`/src/domain/entities/animation.rs`) automatically:
- Advances animation frames based on ACT timing data
- Applies custom palettes if provided
- Supports looping and non-looping animations
- Converts sprite frames to Bevy images with proper transparency

### Usage Patterns

#### Character Customization Preview
```rust
// Preview hair style with custom color
let hair_sprite = asset_server.load("ro://data\\sprite\\인간족\\머리통\\여\\5_여.spr");
let hair_act = asset_server.load("ro://data\\sprite\\인간족\\머리통\\여\\5_여.act");
let hair_palette = asset_server.load("ro://data\\palette\\머리\\5_여_12.pal"); // Red hair

RoSpriteFactory::spawn_hair_preview(
    &mut commands,
    hair_sprite,
    hair_act,
    Some(hair_palette),
    Vec3::new(100.0, 200.0, 0.0),
);
```

#### Item/Equipment Preview
```rust
// Show equipment before equipping
let equipment = asset_server.load("ro://data\\sprite\\악세사리\\여\\여_aura.spr");
let equipment_act = asset_server.load("ro://data\\sprite\\악세사리\\여\\여_aura.act");

RoSpriteFactory::spawn_equipment_preview(
    &mut commands,
    equipment,
    equipment_act,
    Vec3::new(50.0, 50.0, 0.0),
);
```

#### Advanced Control
```rust
// Full control with builder pattern
let sprite_entity = RoSpriteFactory::spawn_from_handles(
    &mut commands,
    sprite_handle,
    act_handle,
    None, // No palette
    position,
    1, // Walking action
);

// Modify controller for custom behavior
if let Some(mut entity_commands) = commands.get_entity(sprite_entity) {
    entity_commands.insert(
        RoAnimationController::new(sprite_handle, act_handle)
            .with_action(2) // Custom action
            .looping(false) // Play once
    );
}
```

### Integration with Existing Systems

- **Character Selection**: Can replace `CharacterSelectionSprite` for simpler implementation
- **Character Creation**: Perfect for hair/face/color previews
- **UI Elements**: Animated icons, effect previews, tooltips
- **In-game Effects**: Buff/debuff visual indicators

The system automatically integrates with:
- Bevy's asset loading (async)
- Existing RO format parsers (SPR, ACT, PAL)
- Sprite rendering pipeline with proper transparency handling
