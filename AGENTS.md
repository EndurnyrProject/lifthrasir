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
The codebase is organized around Bevy's ECS (Entity Component System) pattern with the following main modules:

- **`ro_formats/`** - Parsers for Ragnarok Online file formats (GRF, RSM, RSW, GND, GAT, ACT, SPR)
  - Each format has its own module with parsing logic using the `nom` crate
  - Includes deserialization for game resources and DES encryption support

- **`assets/`** - Asset loading and conversion
  - Custom Bevy asset loaders for RO formats
  - Converters to transform RO formats into Bevy-compatible resources
  - BMP texture loading support

- **`systems/`** - Bevy systems for game logic
  - Camera controls and movement
  - Terrain generation from GND (ground) files  
  - Model spawning from RSM files
  - Enhanced lighting system
  - GRF map extraction and loading
  - Animation systems for sprites and models

- **`components/`** - ECS components
  - `MapLoader` - Manages map asset handles
  - `GrfMapLoader` - Handles GRF file extraction
  - `RsmAnimation` - Model animation data

- **`app/`** - Application plugins
  - `LifthrasirPlugin` - Main application plugin
  - `MapPlugin` - Map loading and rendering plugin

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
// In terrain.rs, we use CELL_SIZE = 10.0 (double RO's 5.0 for better visual scale)
let base_x = x as f32 * CELL_SIZE;
let base_z = y as f32 * CELL_SIZE;
// Heights are multiplied by 5.0 to match the horizontal scale
let height = surface.height[i] * 5.0;
```

#### Model Positioning (RSW → Bevy)
```rust
// From coordinates.rs
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
// From terrain.rs - setup_terrain_camera
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
