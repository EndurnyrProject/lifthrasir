use crate::infrastructure::assets::ro_animation_asset::RoSprite;
use crate::infrastructure::assets::RoAnimationAsset;
use bevy::prelude::*;

// =============================================================================
// PHASE 0.2: SPRITE FACTORY UPDATED
// =============================================================================
// This factory now uses RoSprite + RoAnimationAsset instead of the old
// RoAnimationController + raw asset handles.
//
// The new system loads pre-processed animation assets that contain
// pre-computed textures and frame data.
// =============================================================================

/// Bundle for easily spawning animated RO sprites
#[derive(Bundle)]
pub struct AnimatedRoSpriteBundle {
    pub sprite: RoSprite,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
    pub name: Name,
}

impl Default for AnimatedRoSpriteBundle {
    fn default() -> Self {
        Self {
            sprite: RoSprite::default(),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
            name: Name::new("AnimatedRoSprite"),
        }
    }
}

pub struct RoSpriteFactory;

impl RoSpriteFactory {
    /// Spawn a sprite from a pre-processed animation asset handle
    pub fn spawn_from_animation(
        commands: &mut Commands,
        animation: Handle<RoAnimationAsset>,
        position: Vec3,
        action: u8,
        direction: u8,
    ) -> Entity {
        commands
            .spawn(AnimatedRoSpriteBundle {
                sprite: RoSprite {
                    animation,
                    base_action: action,
                    direction,
                    start_time: 0,
                    speed_factor: 1.0,
                    looping: true,
                    paused: false,
                },
                transform: Transform::from_translation(position),
                name: Name::new(format!("RoSprite_Action{}", action)),
                ..default()
            })
            .id()
    }
}

/// Component to track sprites waiting for asset loading
#[derive(Component)]
pub struct PendingAnimationLoad {
    pub animation_handle: Handle<RoAnimationAsset>,
    pub action: u8,
    pub direction: u8,
}

/// System to convert pending sprite loads into active sprites once assets are ready
pub fn finalize_pending_animation_loads(
    mut commands: Commands,
    pending_query: Query<(Entity, &PendingAnimationLoad)>,
    animations: Res<Assets<RoAnimationAsset>>,
) {
    for (entity, pending) in pending_query.iter() {
        if animations.get(&pending.animation_handle).is_some() {
            commands.entity(entity).insert((
                RoSprite {
                    animation: pending.animation_handle.clone(),
                    base_action: pending.action,
                    direction: pending.direction,
                    start_time: 0,
                    speed_factor: 1.0,
                    looping: true,
                    paused: false,
                },
                Name::new("RoSprite_Active"),
            ));

            commands.entity(entity).remove::<PendingAnimationLoad>();

            info!("Finalized animation load for entity: {:?}", entity);
        }
    }
}
