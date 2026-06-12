# Sprite Rendering Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the sprite rendering system to use a generic `RoSprite<T>` with proper action mapping via traits, replacing the broken `base_action * 8` math.

**Architecture:** Generic `ActionLayout` trait with `PlayerLayout` and `MobLayout` implementations. Body layers publish attach points, head layers read them. Clean system separation: action sync, body sync, head sync, position sync.

**Tech Stack:** Rust, Bevy 0.17, PhantomData for zero-cost generics

---

## Task 1: Create ActionLayout Trait

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/layout/mod.rs`
- Create: `game-engine/src/domain/entities/sprite_rendering/layout/trait_def.rs`

**Step 1: Create layout module directory structure**

```rust
// game-engine/src/domain/entities/sprite_rendering/layout/mod.rs
mod trait_def;
mod player;
mod mob;

pub use trait_def::ActionLayout;
pub use player::PlayerLayout;
pub use mob::MobLayout;
```

**Step 2: Write ActionLayout trait**

```rust
// game-engine/src/domain/entities/sprite_rendering/layout/trait_def.rs
use crate::domain::entities::character::components::visual::{ActionType, Direction};

pub trait ActionLayout: Send + Sync + 'static {
    fn action_offset(action_type: ActionType) -> usize;

    fn calculate_action_index(action_type: ActionType, direction: Direction) -> usize {
        Self::action_offset(action_type) + (direction as usize)
    }

    fn is_looping(action_type: ActionType) -> bool;

    fn validate_action_index(index: usize, total_actions: usize) -> usize {
        if index >= total_actions {
            0
        } else {
            index
        }
    }
}
```

**Step 3: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success (trait defined but not yet used)

**Step 4: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/layout/
git commit -m "feat(sprite): add ActionLayout trait for action index calculation"
```

---

## Task 2: Implement PlayerLayout

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/layout/player.rs`
- Modify: `game-engine/src/domain/entities/sprite_rendering/layout/mod.rs`

**Step 1: Write PlayerLayout with tests**

```rust
// game-engine/src/domain/entities/sprite_rendering/layout/player.rs
use super::ActionLayout;
use crate::domain::entities::character::components::visual::{ActionType, Direction};

pub struct PlayerLayout;

impl ActionLayout for PlayerLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,
            ActionType::Walk => 8,
            ActionType::Sit => 16,
            ActionType::Special => 24,
            ActionType::Hit => 48,
            ActionType::Dead => 64,
            ActionType::Attack => 88,
            ActionType::Cast => 96,
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(action_type, ActionType::Idle | ActionType::Walk | ActionType::Sit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_directions() {
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Idle, Direction::South), 0);
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Idle, Direction::North), 4);
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Idle, Direction::SouthEast), 7);
    }

    #[test]
    fn test_walk_directions() {
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Walk, Direction::South), 8);
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Walk, Direction::North), 12);
    }

    #[test]
    fn test_attack_directions() {
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Attack, Direction::South), 88);
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Attack, Direction::East), 94);
    }

    #[test]
    fn test_hit_directions() {
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Hit, Direction::South), 48);
    }

    #[test]
    fn test_dead_directions() {
        assert_eq!(PlayerLayout::calculate_action_index(ActionType::Dead, Direction::South), 64);
    }

    #[test]
    fn test_looping_actions() {
        assert!(PlayerLayout::is_looping(ActionType::Idle));
        assert!(PlayerLayout::is_looping(ActionType::Walk));
        assert!(PlayerLayout::is_looping(ActionType::Sit));
        assert!(!PlayerLayout::is_looping(ActionType::Attack));
        assert!(!PlayerLayout::is_looping(ActionType::Hit));
        assert!(!PlayerLayout::is_looping(ActionType::Dead));
    }

    #[test]
    fn test_validate_action_index() {
        assert_eq!(PlayerLayout::validate_action_index(10, 100), 10);
        assert_eq!(PlayerLayout::validate_action_index(100, 50), 0);
        assert_eq!(PlayerLayout::validate_action_index(0, 0), 0);
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p game-engine player::tests`
Expected: All tests pass

**Step 3: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/layout/player.rs
git commit -m "feat(sprite): implement PlayerLayout with RO action offsets"
```

---

## Task 3: Implement MobLayout

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/layout/mob.rs`

**Step 1: Write MobLayout with tests**

```rust
// game-engine/src/domain/entities/sprite_rendering/layout/mob.rs
use super::ActionLayout;
use crate::domain::entities::character::components::visual::{ActionType, Direction};

pub struct MobLayout;

impl ActionLayout for MobLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,
            ActionType::Walk => 8,
            ActionType::Attack => 16,
            ActionType::Hit => 24,
            ActionType::Dead => 32,
            ActionType::Sit | ActionType::Cast | ActionType::Special => 0,
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(action_type, ActionType::Idle | ActionType::Walk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mob_idle() {
        assert_eq!(MobLayout::calculate_action_index(ActionType::Idle, Direction::South), 0);
        assert_eq!(MobLayout::calculate_action_index(ActionType::Idle, Direction::North), 4);
    }

    #[test]
    fn test_mob_attack() {
        assert_eq!(MobLayout::calculate_action_index(ActionType::Attack, Direction::South), 16);
        assert_eq!(MobLayout::calculate_action_index(ActionType::Attack, Direction::East), 22);
    }

    #[test]
    fn test_mob_unsupported_actions_fallback_to_idle() {
        assert_eq!(MobLayout::action_offset(ActionType::Sit), 0);
        assert_eq!(MobLayout::action_offset(ActionType::Cast), 0);
    }

    #[test]
    fn test_mob_looping() {
        assert!(MobLayout::is_looping(ActionType::Idle));
        assert!(MobLayout::is_looping(ActionType::Walk));
        assert!(!MobLayout::is_looping(ActionType::Attack));
        assert!(!MobLayout::is_looping(ActionType::Dead));
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p game-engine mob::tests`
Expected: All tests pass

**Step 3: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/layout/mob.rs
git commit -m "feat(sprite): implement MobLayout with simplified action offsets"
```

---

## Task 4: Create New RoSprite<T> Component

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/components/ro_sprite.rs`
- Modify: `game-engine/src/domain/entities/sprite_rendering/components/mod.rs`

**Step 1: Write RoSprite<T> component**

```rust
// game-engine/src/domain/entities/sprite_rendering/components/ro_sprite.rs
use std::marker::PhantomData;

use bevy::prelude::*;

use crate::domain::entities::character::components::visual::{ActionType, Direction};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::infrastructure::assets::ro_animation_asset::{FrameData, RoAnimationAsset};

#[derive(Component, Clone, Debug)]
pub struct RoSpriteGeneric<T: ActionLayout> {
    pub animation: Handle<RoAnimationAsset>,
    pub action_type: ActionType,
    pub direction: Direction,
    pub start_time: u32,
    pub speed_factor: f32,
    _marker: PhantomData<T>,
}

impl<T: ActionLayout> Default for RoSpriteGeneric<T> {
    fn default() -> Self {
        Self {
            animation: Handle::default(),
            action_type: ActionType::Idle,
            direction: Direction::South,
            start_time: 0,
            speed_factor: 1.0,
            _marker: PhantomData,
        }
    }
}

impl<T: ActionLayout> RoSpriteGeneric<T> {
    pub fn new(animation: Handle<RoAnimationAsset>) -> Self {
        Self {
            animation,
            ..Default::default()
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
        let action_index = T::validate_action_index(self.action_index(), animation.actions.len());
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

    pub fn get_static_frame<'a>(&self, animation: &'a RoAnimationAsset) -> Option<&'a FrameData> {
        let action_index = T::validate_action_index(self.action_index(), animation.actions.len());
        let action_data = animation.actions.get(action_index)?;
        action_data.frames.first()
    }
}

// Type aliases
use crate::domain::entities::sprite_rendering::layout::{MobLayout, PlayerLayout};

pub type PlayerSprite = RoSpriteGeneric<PlayerLayout>;
pub type MobSprite = RoSpriteGeneric<MobLayout>;
```

**Step 2: Update components mod.rs**

```rust
// Add to game-engine/src/domain/entities/sprite_rendering/components/mod.rs
mod ro_sprite;

pub use ro_sprite::{RoSpriteGeneric, PlayerSprite, MobSprite};
```

**Step 3: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 4: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/components/
git commit -m "feat(sprite): add generic RoSpriteGeneric<T> with type aliases"
```

---

## Task 5: Create Head/Body Attachment Components

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/components/layers.rs`
- Modify: `game-engine/src/domain/entities/sprite_rendering/components/mod.rs`

**Step 1: Write attachment components**

```rust
// game-engine/src/domain/entities/sprite_rendering/components/layers.rs
use bevy::prelude::*;

#[derive(Component, Default)]
pub struct HeadLayer;

#[derive(Component, Default)]
pub struct BodyAttachPoint(pub Vec2);

#[derive(Component)]
pub struct HeadAttachment {
    pub body_entity: Entity,
}
```

**Step 2: Export from mod.rs**

```rust
// Add to components/mod.rs
mod layers;

pub use layers::{HeadLayer, BodyAttachPoint, HeadAttachment};
```

**Step 3: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 4: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/components/layers.rs
git commit -m "feat(sprite): add HeadLayer, BodyAttachPoint, HeadAttachment components"
```

---

## Task 6: Create Action Sync Systems

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/systems/action_sync.rs`
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/mod.rs`

**Step 1: Write action sync systems**

```rust
// game-engine/src/domain/entities/sprite_rendering/systems/action_sync.rs
use bevy::prelude::*;

use crate::domain::entities::character::components::visual::{ActionType, CharacterDirection};
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::sprite_rendering::components::RoSpriteGeneric;
use crate::domain::entities::sprite_rendering::layout::ActionLayout;

pub fn sync_sprite_action<T: ActionLayout>(
    time: Res<Time>,
    mut query: Query<(&AnimationState, &mut RoSpriteGeneric<T>), Changed<AnimationState>>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (state, mut ro_sprite) in query.iter_mut() {
        let action_type: ActionType = (*state).into();
        ro_sprite.set_action(action_type, game_time_ms);
    }
}

pub fn sync_sprite_direction<T: ActionLayout>(
    mut query: Query<(&CharacterDirection, &mut RoSpriteGeneric<T>), Changed<CharacterDirection>>,
) {
    for (char_dir, mut ro_sprite) in query.iter_mut() {
        ro_sprite.set_direction(char_dir.facing);
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 3: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/systems/action_sync.rs
git commit -m "feat(sprite): add sync_sprite_action and sync_sprite_direction systems"
```

---

## Task 7: Create Body Sync System

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/systems/body_sync.rs`

**Step 1: Write body sync system**

```rust
// game-engine/src/domain/entities/sprite_rendering/systems/body_sync.rs
use bevy::prelude::*;

use crate::domain::entities::sprite_rendering::components::{
    BodyAttachPoint, HeadLayer, RenderLayer, RoSpriteGeneric,
};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;

pub fn sync_body_layer<T: ActionLayout>(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    parent_query: Query<&RoSpriteGeneric<T>>,
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

        if let Some(part) = frame.parts.first() {
            if let Some(texture) = animation.textures.get(part.texture_index) {
                sprite.image = texture.clone();
            }
        }

        if let Some(ap) = frame.attach_point {
            attach_point.0 = ap;
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 3: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/systems/body_sync.rs
git commit -m "feat(sprite): add sync_body_layer system with attach point publishing"
```

---

## Task 8: Create Head Sync Systems

**Files:**
- Create: `game-engine/src/domain/entities/sprite_rendering/systems/head_sync.rs`

**Step 1: Write head sync systems**

```rust
// game-engine/src/domain/entities/sprite_rendering/systems/head_sync.rs
use bevy::prelude::*;

use crate::domain::entities::character::components::visual::{ActionType, Direction};
use crate::domain::entities::sprite_rendering::components::{
    BodyAttachPoint, HeadAttachment, HeadLayer, RenderLayer, RoSpriteGeneric,
};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::infrastructure::assets::ro_animation_asset::{FrameData, RoAnimationAsset};

fn get_static_head_frame(animation: &RoAnimationAsset, direction: Direction) -> Option<&FrameData> {
    let action_index = direction as usize;
    let action_data = animation.actions.get(action_index)?;
    action_data.frames.first()
}

pub fn sync_head_layer<T: ActionLayout>(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    parent_query: Query<&RoSpriteGeneric<T>>,
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

pub fn sync_head_position<T: ActionLayout>(
    time: Res<Time>,
    body_query: Query<&BodyAttachPoint>,
    animations: Res<Assets<RoAnimationAsset>>,
    parent_query: Query<&RoSpriteGeneric<T>>,
    mut head_query: Query<
        (&HeadAttachment, &RenderLayer, &Parent, &mut Transform),
        With<HeadLayer>,
    >,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

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

        let frame = if ro_sprite.action_type == ActionType::Idle {
            get_static_head_frame(animation, ro_sprite.direction)
        } else {
            ro_sprite.get_frame(animation, game_time_ms)
        };

        let Some(head_attach) = frame.and_then(|f| f.attach_point) else {
            continue;
        };

        let offset = body_attach.0 - head_attach;
        transform.translation.x = offset.x;
        transform.translation.y = offset.y;
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 3: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/systems/head_sync.rs
git commit -m "feat(sprite): add sync_head_layer and sync_head_position systems"
```

---

## Task 9: Wire Up Systems Module

**Files:**
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/mod.rs`

**Step 1: Export new systems and update module**

Update `systems/mod.rs` to export the new systems. Keep existing systems temporarily for migration.

**Step 2: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 3: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/systems/mod.rs
git commit -m "feat(sprite): wire up new sync systems in module"
```

---

## Task 10: Update Sprite Rendering Plugin

**Files:**
- Modify: `game-engine/src/app/sprite_rendering_domain_plugin.rs`

**Step 1: Register new system sets and systems**

Add the new systems to the plugin with proper ordering:
- AnimationSync (action + direction sync)
- BodyUpdate (body layer sync)
- HeadUpdate (head layer sync)
- PositionUpdate (head position sync)
- OrphanCleanup (existing)

**Step 2: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 3: Run the game to test**

Run: `cargo run -p game-engine` (or tauri dev)
Expected: Game runs, sprites render correctly

**Step 4: Commit**

```bash
git add game-engine/src/app/sprite_rendering_domain_plugin.rs
git commit -m "feat(sprite): register new sync systems in plugin with ordering"
```

---

## Task 11: Update Spawn System

**Files:**
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/spawn.rs`

**Step 1: Update spawn to use new components**

- Use `PlayerSprite` or `MobSprite` instead of old `RoSprite`
- Add `BodyAttachPoint` to body layers
- Add `HeadLayer` and `HeadAttachment` to head layers

**Step 2: Verify it compiles**

Run: `cargo check -p game-engine`
Expected: Success

**Step 3: Run and test visually**

Run: `cargo run -p game-engine`
Expected: Player sprites render with head attached correctly

**Step 4: Commit**

```bash
git add game-engine/src/domain/entities/sprite_rendering/systems/spawn.rs
git commit -m "feat(sprite): update spawn system to use new component architecture"
```

---

## Task 12: Remove Old Code

**Files:**
- Delete: `game-engine/src/domain/entities/character/components/action_mapping.rs`
- Modify: `game-engine/src/domain/entities/character/components/mod.rs`
- Modify: `game-engine/src/infrastructure/assets/ro_animation_asset.rs`
- Delete old systems from: `game-engine/src/domain/entities/sprite_rendering/systems/update.rs`

**Step 1: Remove old action_mapping.rs**

Delete the file and remove from mod.rs exports.

**Step 2: Clean up old RoSprite**

Remove `base_action`, `actual_action_index()`, `get_head_idle_frame()` from the old RoSprite in ro_animation_asset.rs (or delete entirely if fully replaced).

**Step 3: Remove old update.rs systems**

Delete `animation_state_to_action`, `sync_layer_sprites`, old `sync_sprite_action`.

**Step 4: Verify everything compiles**

Run: `cargo check -p game-engine`
Expected: Success with no warnings about unused code

**Step 5: Run full test suite**

Run: `cargo test -p game-engine`
Expected: All tests pass

**Step 6: Final visual test**

Run: `cargo run -p game-engine`
Expected: Game runs, all sprite animations work correctly

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor(sprite): remove old action mapping and RoSprite implementation"
```

---

## Task 13: Final Cleanup and Documentation

**Step 1: Run clippy**

Run: `cargo clippy -p game-engine`
Expected: No warnings

**Step 2: Run fmt**

Run: `cargo fmt`
Expected: Code formatted

**Step 3: Commit any formatting changes**

```bash
git add -A
git commit -m "chore: format code and fix clippy warnings"
```

**Step 4: Squash/rebase commits if desired**

Optional: Clean up commit history before merging.
