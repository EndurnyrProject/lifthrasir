use super::components::RoAnimationController;
use crate::infrastructure::assets::loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
use bevy::prelude::*;

/// Bundle for easily spawning animated RO sprites
#[derive(Bundle)]
pub struct AnimatedRoSpriteBundle {
    pub controller: RoAnimationController,
    pub sprite: Sprite,
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
            controller: RoAnimationController::new(Handle::default(), Handle::default()),
            sprite: Sprite::default(),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
            name: Name::new("AnimatedRoSprite"),
        }
    }
}

/// Factory for creating animated RO sprites with various configurations
pub struct RoSpriteFactory;

impl RoSpriteFactory {
    /// Spawn a sprite from pre-loaded asset handles (immediate rendering)
    /// Use this when you already have the asset handles loaded
    pub fn spawn_from_handles(
        commands: &mut Commands,
        sprite_handle: Handle<RoSpriteAsset>,
        act_handle: Handle<RoActAsset>,
        palette_handle: Option<Handle<RoPaletteAsset>>,
        position: Vec3,
        action_index: usize,
    ) -> Entity {
        let mut controller = RoAnimationController::new(sprite_handle, act_handle)
            .with_action(action_index)
            .looping(true);

        if let Some(palette) = palette_handle {
            controller = controller.with_palette(palette);
        }

        commands
            .spawn(AnimatedRoSpriteBundle {
                controller,
                transform: Transform::from_translation(position),
                name: Name::new(format!("RoSprite_Action{}", action_index)),
                ..default()
            })
            .id()
    }

    /// Spawn a sprite from paths (triggers async asset loading via AssetServer)
    /// Use this when you need to load sprites by file path
    /// Paths should already include "ro://" prefix
    /// Returns entity ID - sprite will render once assets are loaded
    pub fn spawn_from_paths(
        commands: &mut Commands,
        asset_server: &AssetServer,
        sprite_path: String,
        act_path: String,
        palette_path: Option<String>,
        position: Vec3,
        action_index: usize,
    ) -> Entity {
        // Load assets using AssetServer (paths should already have "ro://" prefix)
        let sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
        let act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
        let palette_handle = palette_path
            .as_ref()
            .map(|path| asset_server.load::<RoPaletteAsset>(path));

        // Create entity with loading handles
        let entity = commands
            .spawn((
                Transform::from_translation(position),
                GlobalTransform::default(),
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
                Name::new(format!("RoSprite_Loading_{}", sprite_path)),
                PendingSpriteLoad {
                    sprite_handle: sprite_handle.clone(),
                    act_handle: act_handle.clone(),
                    palette_handle: palette_handle.clone(),
                    action_index,
                },
            ))
            .id();

        entity
    }
}

/// Component to track sprites waiting for asset loading
#[derive(Component)]
pub struct PendingSpriteLoad {
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub act_handle: Handle<RoActAsset>,
    pub palette_handle: Option<Handle<RoPaletteAsset>>,
    pub action_index: usize,
}

/// System to convert pending sprite loads into active sprites once assets are ready
pub fn finalize_pending_sprite_loads(
    mut commands: Commands,
    pending_query: Query<(Entity, &PendingSpriteLoad)>,
    asset_server: Res<AssetServer>,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
) {
    for (entity, pending) in pending_query.iter() {
        // Check if all required assets are loaded
        let sprite_loaded = sprites.get(&pending.sprite_handle).is_some();
        let act_loaded = actions.get(&pending.act_handle).is_some();

        if sprite_loaded && act_loaded {
            // All assets loaded - create controller
            let mut controller = RoAnimationController::new(
                pending.sprite_handle.clone(),
                pending.act_handle.clone(),
            )
            .with_action(pending.action_index)
            .looping(true);

            if let Some(palette) = &pending.palette_handle {
                controller = controller.with_palette(palette.clone());
            }

            // Update entity with animation controller
            commands.entity(entity).insert((
                controller,
                Sprite::default(),
                Name::new("RoSprite_Active"),
            ));

            // Remove pending component
            commands.entity(entity).remove::<PendingSpriteLoad>();

            info!("Finalized sprite load for entity: {:?}", entity);
        }
    }
}
