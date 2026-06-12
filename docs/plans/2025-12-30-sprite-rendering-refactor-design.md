# Sprite Rendering System Refactor

## Problem Statement

The current `update.rs` in sprite_rendering has several issues:

1. **Incorrect action index math**: `RoSprite` stores `base_action` as 0-4 and calculates `base_action * 8 + direction`, but RO's actual layout has gaps (Idle=0, Walk=8, Hit=48, Dead=64, Attack=88)
2. **Ignores existing action_mapping.rs**: Well-designed mapping exists in `character/components/action_mapping.rs` but isn't used
3. **Hardcoded hacks**: Magic numbers like `is_idle = ro_sprite.base_action == 0`
4. **Mixed concerns**: Texture syncing interleaved with attachment point calculations
5. **Raw types**: Uses `u8` for action and direction instead of enums

## Design Goals

- Unified `RoSprite` that works for both players and mobs
- Use proper action mapping via trait-based layout system
- Clean separation of body/head rendering concerns
- Type-safe enums instead of raw integers
- Match reference.xml implementation (body as parent, head as child)

## Core Trait: ActionLayout

```rust
pub trait ActionLayout: Send + Sync + 'static {
    /// Get the base offset for an action type in the ACT file
    fn action_offset(action_type: ActionType) -> usize;

    /// Calculate full action index (offset + direction)
    fn calculate_action_index(action_type: ActionType, direction: Direction) -> usize {
        Self::action_offset(action_type) + (direction as usize)
    }

    /// Determine if an action should loop
    fn is_looping(action_type: ActionType) -> bool;

    /// Validate and clamp action index to available actions
    fn validate_action_index(index: usize, total_actions: usize) -> usize {
        if index >= total_actions {
            0 // Fallback to idle south
        } else {
            index
        }
    }
}
```

### PlayerLayout Implementation

```rust
pub struct PlayerLayout;

impl ActionLayout for PlayerLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,      // 0 * 8
            ActionType::Walk => 8,      // 1 * 8
            ActionType::Sit => 16,      // 2 * 8
            ActionType::Special => 24,  // 3 * 8 (pickup)
            ActionType::Hit => 48,      // 6 * 8
            ActionType::Dead => 64,     // 8 * 8
            ActionType::Attack => 88,   // 11 * 8
            ActionType::Cast => 96,     // 12 * 8
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(action_type, ActionType::Idle | ActionType::Walk | ActionType::Sit)
    }
}
```

### MobLayout Implementation

```rust
pub struct MobLayout;

impl ActionLayout for MobLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,
            ActionType::Walk => 8,
            ActionType::Attack => 16,
            ActionType::Hit => 24,
            ActionType::Dead => 32,
            // Mobs don't have these - fallback to idle
            ActionType::Sit | ActionType::Cast | ActionType::Special => 0,
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(action_type, ActionType::Idle | ActionType::Walk)
    }
}
```

## RoSprite Component

```rust
#[derive(Component, Clone, Debug)]
pub struct RoSprite<T: ActionLayout> {
    pub animation: Handle<RoAnimationAsset>,
    pub action_type: ActionType,
    pub direction: Direction,
    pub start_time: u32,
    pub speed_factor: f32,
    _marker: PhantomData<T>,
}

impl<T: ActionLayout> RoSprite<T> {
    pub fn new(animation: Handle<RoAnimationAsset>) -> Self {
        Self {
            animation,
            action_type: ActionType::Idle,
            direction: Direction::South,
            start_time: 0,
            speed_factor: 1.0,
            _marker: PhantomData,
        }
    }

    pub fn action_index(&self) -> usize {
        T::calculate_action_index(self.action_type, self.direction)
    }

    pub fn is_looping(&self) -> bool {
        T::is_looping(self.action_type)
    }

    pub fn set_action(&mut self, action_type: ActionType, game_time_ms: u32) {
        if self.action_type != action_type {
            self.action_type = action_type;
            self.start_time = game_time_ms;
        }
    }

    pub fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }

    pub fn get_frame<'a>(
        &self,
        animation: &'a RoAnimationAsset,
        game_time_ms: u32,
    ) -> Option<&'a FrameData> {
        let action_index = T::validate_action_index(
            self.action_index(),
            animation.actions.len()
        );
        let action_data = animation.actions.get(action_index)?;

        if action_data.frames.is_empty() {
            return None;
        }

        let elapsed = game_time_ms.wrapping_sub(self.start_time);
        let delay = (action_data.delay_ms * self.speed_factor).max(1.0);
        let frame_time = (elapsed as f32 / delay) as usize;

        let frame_index = if self.is_looping() {
            frame_time % action_data.frames.len()
        } else {
            frame_time.min(action_data.frames.len().saturating_sub(1))
        };

        action_data.frames.get(frame_index)
    }
}

// Type aliases for convenience
pub type PlayerSprite = RoSprite<PlayerLayout>;
pub type MobSprite = RoSprite<MobLayout>;
```

## Head/Body Attachment Components

```rust
/// Marker for head sprite layers - uses static frame during idle
#[derive(Component, Default)]
pub struct HeadLayer;

/// Body publishes its attach point each frame for head to read
#[derive(Component, Default)]
pub struct BodyAttachPoint(pub Vec2);

/// Head stores reference to body entity for attach point lookup
#[derive(Component)]
pub struct HeadAttachment {
    pub body_entity: Entity,
}
```

## Systems

### Action/Direction Sync (runs on component change)

```rust
pub fn sync_sprite_action<T: ActionLayout>(
    time: Res<Time>,
    mut query: Query<(&AnimationState, &mut RoSprite<T>), Changed<AnimationState>>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    for (state, mut ro_sprite) in query.iter_mut() {
        let action_type: ActionType = (*state).into();
        ro_sprite.set_action(action_type, game_time_ms);
    }
}

pub fn sync_sprite_direction<T: ActionLayout>(
    mut query: Query<(&CharacterDirection, &mut RoSprite<T>), Changed<CharacterDirection>>,
) {
    for (char_dir, mut ro_sprite) in query.iter_mut() {
        ro_sprite.set_direction(char_dir.facing);
    }
}
```

### Body Layer Sync

```rust
pub fn sync_body_layer<T: ActionLayout>(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    parent_query: Query<&RoSprite<T>>,
    mut layer_query: Query<
        (&RenderLayer, &Parent, &mut Sprite, &mut BodyAttachPoint),
        Without<HeadLayer>,
    >,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (layer, parent, mut sprite, mut attach_point) in layer_query.iter_mut() {
        let Ok(ro_sprite) = parent_query.get(parent.get()) else {
            continue;
        };
        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };
        let Some(frame) = ro_sprite.get_frame(animation, game_time_ms) else {
            continue;
        };

        // Update texture
        if let Some(part) = frame.parts.first() {
            if let Some(texture) = animation.textures.get(part.texture_index) {
                sprite.image = texture.clone();
            }
        }

        // Publish attach point for head
        if let Some(ap) = frame.attach_point {
            attach_point.0 = ap;
        }
    }
}
```

### Head Layer Sync

```rust
pub fn sync_head_layer<T: ActionLayout>(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    parent_query: Query<&RoSprite<T>>,
    mut head_query: Query<(&RenderLayer, &Parent, &mut Sprite), With<HeadLayer>>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (layer, parent, mut sprite) in head_query.iter_mut() {
        let Ok(ro_sprite) = parent_query.get(parent.get()) else {
            continue;
        };
        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        // Use static frame during idle to prevent doridori
        let frame = if ro_sprite.action_type == ActionType::Idle {
            get_static_head_frame(animation, ro_sprite.direction)
        } else {
            ro_sprite.get_frame(animation, game_time_ms)
        };

        let Some(frame) = frame else {
            continue;
        };

        if let Some(part) = frame.parts.first() {
            if let Some(texture) = animation.textures.get(part.texture_index) {
                sprite.image = texture.clone();
            }
        }
    }
}

fn get_static_head_frame(animation: &RoAnimationAsset, direction: Direction) -> Option<&FrameData> {
    // Head idle: action_index = direction (0-7)
    let action_index = direction as usize;
    let action_data = animation.actions.get(action_index)?;
    action_data.frames.first()
}
```

### Head Position Sync

```rust
pub fn sync_head_position(
    body_query: Query<&BodyAttachPoint>,
    animations: Res<Assets<RoAnimationAsset>>,
    parent_query: Query<&RoSprite<PlayerLayout>>,
    mut head_query: Query<
        (&HeadAttachment, &RenderLayer, &Parent, &mut Transform),
        With<HeadLayer>,
    >,
) {
    for (attachment, layer, parent, mut transform) in head_query.iter_mut() {
        let Ok(body_attach) = body_query.get(attachment.body_entity) else {
            continue;
        };
        let Ok(ro_sprite) = parent_query.get(parent.get()) else {
            continue;
        };
        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        // Get head's attach point from current frame
        let frame = if ro_sprite.action_type == ActionType::Idle {
            get_static_head_frame(animation, ro_sprite.direction)
        } else {
            let game_time_ms = 0; // TODO: pass actual time
            ro_sprite.get_frame(animation, game_time_ms)
        };

        let Some(head_attach) = frame.and_then(|f| f.attach_point) else {
            continue;
        };

        // Formula from reference: offset = -head_attach + body_attach
        let offset = body_attach.0 - head_attach;
        transform.translation.x = offset.x;
        transform.translation.y = offset.y;
    }
}
```

### System Ordering

```rust
app.configure_sets(Update, (
    SpriteRenderingSystems::AnimationSync,
    SpriteRenderingSystems::BodyUpdate,
    SpriteRenderingSystems::HeadUpdate,
    SpriteRenderingSystems::PositionUpdate,
    SpriteRenderingSystems::OrphanCleanup,
).chain());
```

## File Organization

```
game-engine/src/domain/entities/sprite_rendering/
├── mod.rs
├── components/
│   ├── mod.rs
│   ├── ro_sprite.rs         # RoSprite<T> component
│   ├── layers.rs            # HeadLayer, BodyAttachPoint, HeadAttachment
│   └── render_layer.rs      # RenderLayer (existing)
├── layout/
│   ├── mod.rs
│   ├── trait.rs             # ActionLayout trait
│   ├── player.rs            # PlayerLayout implementation
│   └── mob.rs               # MobLayout implementation
├── systems/
│   ├── mod.rs
│   ├── action_sync.rs       # sync_sprite_action, sync_sprite_direction
│   ├── body_sync.rs         # sync_body_layer
│   ├── head_sync.rs         # sync_head_layer, sync_head_position
│   ├── spawn.rs             # existing spawn logic (updated)
│   └── cleanup.rs           # cleanup_orphaned_sprites
└── tags.rs                  # existing layer tags
```

## Migration Path

1. Create `layout/` module with trait and implementations
2. Create new `RoSprite<T>` in `components/ro_sprite.rs`
3. Add `HeadLayer`, `BodyAttachPoint`, `HeadAttachment` components
4. Implement new systems alongside old ones
5. Update spawn system to use new components
6. Test with both systems running
7. Remove old systems once new ones verified working
8. Delete `character/components/action_mapping.rs` (logic moved to layout/)
9. Delete old `update.rs` code

## Files to Delete

- `game-engine/src/domain/entities/character/components/action_mapping.rs`
- Old `animation_state_to_action` function in update.rs
- `RoSprite.base_action` field and `actual_action_index()` method from ro_animation_asset.rs
