# Ragnarok Online ACT File Format

## Overview

ACT (Action) files define sprite animations for Ragnarok Online. Each ACT file contains multiple actions (animation sequences), where each action consists of multiple animation frames with timing, positioning, scaling, rotation, and layering information.

ACT files work in conjunction with SPR (Sprite) files:
- **SPR files** contain the actual image frames (texture atlas)
- **ACT files** define how and when to display those frames (animation sequences)

## Action Index System

Actions in RO use a **base index multiplied by 8** for 8-directional sprites, or exist as single actions for omnidirectional sprites. Each action represents a different animation state (idle, walk, attack, etc.).

## Player Character Actions

Player characters use the most complex action set with directional variants:

| Base Action | Action Indices | Description |
|-------------|----------------|-------------|
| **0** | 0-7 | **Idle** (1 per direction) |
| **1** | 8-15 | **Walk** (1×8 directions) |
| **2** | 16-23 | **Sit** (1×8 directions) |
| **3** | 24-31 | **Pick Up** (1×8 directions) |
| **4** | 32-39 | **Standby** (ready pose, 1×8 directions) |
| **5** | 40-47 | *Reserved/Unused* |
| **6** | 48-55 | **Hit** (damage taken, 1×8 directions) |
| **7** | 56-63 | **Freeze 1** (frozen state, 1×8 directions) |
| **8** | 64-71 | **Dead** (death animation, 1×8 directions) |
| **9** | 72-79 | **Freeze 2** (alternate frozen, 1×8 directions) |
| **10** | 80-87 | **Attack 2** (alternate attack, 1×8 directions) |
| **11** | 88-95 | **Attack 1 / Attack 3** (primary attack, 1×8 directions) |
| **12** | 96-103 | **Casting** (spell cast, 1×8 directions) |

### Direction Encoding

For 8-directional actions, the formula is:
```rust
actual_index = base_action * 8 + direction

// Directions:
// 0 = South, 1 = SouthWest, 2 = West, 3 = NorthWest
// 4 = North, 5 = NorthEast, 6 = East, 7 = SouthEast
```

**Example**: Walk action facing East:
- Base action = 1 (Walk)
- Direction = 6 (East)
- Actual index = 1 × 8 + 6 = **14**

## Monster Actions

Monsters and pets use a simpler action set:

| Base Action | Action Indices | Description |
|-------------|----------------|-------------|
| **0** | 0-7 | **Idle** (1×8 directions) |
| **1** | 8-15 | **Walk** (1×8 directions) |
| **2** | 16-23 | **Attack 1 / Attack 2 / Attack 3** (all map to same, 1×8 directions) |
| **3** | 24-31 | **Hit** (damage taken, 1×8 directions) |
| **4** | 32-39 | **Dead** (death animation, 1×8 directions) |

### Monster2 Type

Some monsters (SpriteType::Monster2) have an additional attack:

| Base Action | Action Indices | Description |
|-------------|----------------|-------------|
| **5** | 40-47 | **Attack 2** (alternate attack, 1×8 directions) |

### Pet-Specific Actions

Pets have additional performance actions:

| Base Action | Action Indices | Description |
|-------------|----------------|-------------|
| **5** | 40-47 | **Special** (special ability, 1×8 directions) |
| **6** | 48-55 | **Performance 1** (emote/action 1, 1×8 directions) |
| **7** | 56-63 | **Performance 2** (emote/action 2, 1×8 directions) |
| **8** | 64-71 | **Performance 3** (emote/action 3, 1×8 directions) |

## NPC Actions

NPCs and effects typically use omnidirectional (non-directional) actions:

| Base Action | Action Indices | Description |
|-------------|----------------|-------------|
| **0** | 0 | **Idle** (omnidirectional, single frame or looping) |

Some ActionNPCs support additional actions:

| Base Action | Action Indices | Description |
|-------------|----------------|-------------|
| **1** | 8-15 | **Walk** (1×8 directions) |
| **2** | 16-23 | **Hit** (1×8 directions) |
| **3** | 24-31 | **Attack 1** (1×8 directions) |

## Head Sprites and Doridori

**Head sprites have a special 3× frame multiplier for the "doridori" head-nodding animation**:

- Body sprites at Idle (action 0): **8 frames** (1 per direction)
- Head sprites at Idle (action 0): **24 frames** (8 directions × 3 head positions)

The 3 head positions are:
1. **headDir 0**: Looking forward (default, used during normal gameplay)
2. **headDir 1**: Looking left (doridori left)
3. **headDir 2**: Looking right (doridori right)

### Frame Layout

```
Direction:     S   SW   W   NW   N   NE   E   SE
headDir 0:    [0] [1] [2] [3] [4] [5] [6] [7]    ← Normal (frames 0-7)
headDir 1:    [8] [9] [10][11][12][13][14][15]   ← Left nod (frames 8-15)
headDir 2:    [16][17][18][19][20][21][22][23]   ← Right nod (frames 16-23)
```

### Implementation Note

To keep heads synchronized with body animations during idle, we clamp head frames to the first variant:

```rust
// For head layers at idle (action 0)
let head_frames_per_variant = animation_count / 3; // Usually 8
let mapped_frame = body_frame % head_frames_per_variant;
```

This ensures frame 3 of body (facing NW) maps to frame 3 of head (also facing NW with headDir 0), not frame 3 of head (which would be NW with headDir 0).

## Animation Frame Structure

Each animation frame in an ACT action contains one or more **layers**:

```rust
pub struct Animation {
    pub layers: Vec<Layer>,
}

pub struct Layer {
    pub sprite_index: i32,      // Index into SPR file (-1 = invisible)
    pub pos: [i32; 2],          // Offset from character anchor [x, y] in pixels
    pub scale: [f32; 2],        // Scale multiplier [x, y]
    pub angle: i32,             // Rotation in degrees
    pub color: [f32; 4],        // RGBA color multiplier [r, g, b, a]
    pub sprite_type: u32,       // Type identifier
    pub width: u32,             // Width (usually from SPR)
    pub height: u32,            // Height (usually from SPR)
}
```

### Sprite Index -1 Handling

A `sprite_index` of **-1** indicates:
- No sprite should be rendered for this layer
- The layer is invisible or disabled
- Some implementations use index **0** as a fallback

**Our Implementation**:
```rust
let sprite_index = if layer.sprite_index < 0 {
    0  // Use first sprite as fallback
} else {
    layer.sprite_index as usize
};
```

## Action Sequence Properties

Each action sequence contains:

```rust
pub struct ActionSequence {
    pub animations: Vec<Animation>,  // All frames for this action
    pub delay: f32,                  // Milliseconds between frames
}
```

- **animations**: All animation frames for this action (e.g., 8 frames for idle, one per direction)
- **delay**: Time in **milliseconds** to display each frame before advancing

### Looping Behavior

Actions have different looping characteristics:

**Looping Actions** (repeat continuously):
- Idle
- Walk
- Sit
- Casting
- Freeze1, Freeze2
- Dead (stays on last frame)

**Non-Looping Actions** (play once and stop):
- Attack1, Attack2, Attack3
- Hit
- PickUp
- Special
- Performance1, Performance2, Performance3

## Direction Queries

To determine if an action uses 8-directional sprites:

```rust
pub fn is_8_direction(sprite_type: SpriteType, motion: SpriteMotion) -> bool {
    match sprite_type {
        SpriteType::Player | SpriteType::Head | SpriteType::Headgear | SpriteType::Npc => {
            match motion {
                SpriteMotion::Idle | SpriteMotion::Sit | SpriteMotion::Walk => true,
                _ => false,
            }
        }
        _ => false,
    }
}
```

## Common Pitfalls

1. **Assuming action indices are sequential**: They're not! Walking is action 8-15, not 1-7.

2. **Forgetting the × 8 multiplier**: Base actions are multiplied by 8 for directions.

3. **Mishandling head sprites**: Heads have 3× frames for doridori; must be clamped during idle.

4. **Not checking sprite_index bounds**: Always validate against SPR frame count.

5. **Ignoring -1 sprite indices**: These indicate invisible layers, not errors.

6. **Using wrong delay units**: ACT delays are in **milliseconds**, not seconds.

7. **Mixing up action 0**: Action 0 is **always** Idle, never assume it's something else.

## File Locations

Typical ACT file paths in RO:
- **Player bodies**: `data/sprite/인간족/몸통/{sex}/{job}_{sex}.act`
- **Player heads**: `data/sprite/인간족/머리통/{sex}/{style}_{sex}.act`
- **Monsters**: `data/sprite/몬스터/{name}.act`
- **NPCs**: `data/sprite/npc/{name}.act`
- **Equipment**: `data/sprite/악세사리/{type}/{item}_{sex}.act`

## Related Files

- **SPR Format**: Texture atlas containing actual sprite frames
- **PAL Format**: Color palettes for recoloring sprites (hair, clothing)
- **GRF Archives**: Container format for game assets

---

**Last Updated**: 2025-10-13
