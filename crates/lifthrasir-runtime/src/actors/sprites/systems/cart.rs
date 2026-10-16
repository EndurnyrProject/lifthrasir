use crate::actors::sprites::components::{CartLayer, PlayerSprite, RenderLayer};
use crate::actors::{ActionType, Direction};
use crate::rendering::SpriteRenderingSystems;
use crate::rendering::billboard::{Billboard, SharedSpriteQuad};
use crate::rendering::constants::SPRITE_WORLD_SCALE;
use crate::rendering::layers::{
    LAYER_CART, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER, layer_depth_bias, layer_order,
};
use crate::rendering::set_layer_texture;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use lifthrasir_assets::animation::RoAnimationAsset;
use lifthrasir_assets::{
    SpriteProcessingSettings,
    animation::RoAnimationProcessor,
    loaders::{RoActAsset, RoSpriteAsset},
    paths,
};

/// SPR/ACT handles still loading for a cart child. Kept on the child itself so
/// the cart never touches the shared `PendingAnimations` queue (whose whole-queue
/// drainers raced it); `finalize_cart_layer` polls these and removes the marker.
#[derive(Component)]
pub struct CartAnimationPending {
    spr: Handle<RoSpriteAsset>,
    act: Handle<RoActAsset>,
}

/// Number of ACT layers per cart frame. Every action/frame of 손수레.act
/// composes exactly two: the wheel piece then the cart body on top.
const CART_ACT_PARTS: usize = 2;

/// Spawns the cart children now (one quad per ACT part, so the mount is
/// observable immediately) with empty animations; `finalize_cart_layer` fills
/// them once the SPR/ACT load. The children start hidden to avoid blank quads
/// flashing before their first texture.
pub fn spawn_cart_layer(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let z_offset = layer_order(LAYER_CART) as f32 * Z_OFFSET_PER_LAYER;

    for part in 0..CART_ACT_PARTS {
        let material = materials.add(StandardMaterial {
            base_color_texture: None,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            // Later ACT parts stack on top within the cart's own bias slot.
            depth_bias: layer_depth_bias(LAYER_CART) + part as f32 * 0.01,
            ..default()
        });

        // Later ACT layers draw on top, so give each part a tiny z step.
        let part_z = z_offset + part as f32 * 0.001;

        commands.spawn((
            Mesh3d(shared_quad.mesh.clone()),
            MeshMaterial3d(material),
            Billboard,
            RenderLayer::body(Handle::default(), LAYER_CART, Vec::new()),
            CartLayer { part },
            CartAnimationPending {
                spr: asset_server.load(paths::cart_sprite_path()),
                act: asset_server.load(paths::cart_action_path()),
            },
            Transform::from_translation(Vec3::new(0.0, SPRITE_BASE_Y_OFFSET, part_z)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            ChildOf(parent),
        ));
    }
}

/// Fills the cart child's animation handle + textures once its SPR/ACT finish
/// loading, polling the handles carried by [`CartAnimationPending`]. The cart
/// deliberately bypasses `PendingAnimations`: that queue has whole-queue
/// drainers (`finalize_render_layers`, `finalize_equipment_layers`) that
/// consumed the cart's completion, leaving the layer permanently hidden.
#[auto_add_system(
    plugin = crate::actors::ActorRenderPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AssetPopulation)
)]
pub fn finalize_cart_layer(
    mut commands: Commands,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<SpriteProcessingSettings>,
    mut cart_layers: Query<(Entity, &CartAnimationPending, &mut RenderLayer), With<CartLayer>>,
) {
    for (entity, pending, mut render_layer) in &mut cart_layers {
        let (Some(sprite), Some(action)) = (sprites.get(&pending.spr), actions.get(&pending.act))
        else {
            continue;
        };

        let animation = RoAnimationProcessor::process(
            &sprite.sprite,
            &action.action,
            None,
            LAYER_CART,
            &mut images,
            settings.upscaling,
        );

        render_layer.textures = animation.textures.clone();
        render_layer.animation = animations.add(animation);
        commands.entity(entity).remove::<CartAnimationPending>();
        debug!("cart: animation finalized for {entity:?}");
    }
}

type CartLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static CartLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut Visibility,
    ),
>;

/// How far behind the character the cart trails, in world units on the ground
/// plane (one GAT cell = 5.0). The cart ACT carries no anchor positions, so the
/// pull-behind placement is ours; the offset is opposite the facing direction,
/// which also gives correct depth order for free (behind the body when facing
/// the camera, in front when facing away).
const CART_BACK_DISTANCE: f32 = 7.0;

/// Ground-plane (x, z) offset pointing behind a unit facing `direction`.
/// World axes: South = +Z, East = +X. Diagonals keep full per-axis magnitude
/// (a grid-diagonal step, not a normalized vector) so the cart lands on the
/// visually adjacent back cell.
fn cart_behind_offset(direction: Direction) -> Vec2 {
    match direction {
        Direction::South => Vec2::new(0.0, -1.0),
        Direction::SouthWest => Vec2::new(1.0, -1.0),
        Direction::West => Vec2::new(1.0, 0.0),
        Direction::NorthWest => Vec2::new(1.0, 1.0),
        Direction::North => Vec2::new(0.0, 1.0),
        Direction::NorthEast => Vec2::new(-1.0, 1.0),
        Direction::East => Vec2::new(-1.0, 0.0),
        Direction::SouthEast => Vec2::new(-1.0, -1.0),
    }
}

/// Drives each cart quad per frame off its parent's `PlayerSprite`: the cart
/// ACT is direction-only (8 actions), so the parent's facing picks the action
/// and the wheel frames animate on the cart's own delay while walking.
/// Positions like the body (raw layer position), then pulls the quad behind
/// the character on the ground plane.
#[auto_add_system(
    plugin = crate::actors::ActorRenderPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_cart_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<&PlayerSprite>,
    mut cart_query: CartLayerQuery,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (layer, cart, child_of, material_handle, mut transform, mut visibility) in
        cart_query.iter_mut()
    {
        let Ok(ro_sprite) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        // The cart ACT is direction-only (one action per facing), unlike the
        // body's action-type x direction grid, so index it by facing directly.
        // The frames roll the wheels: animate them on the cart's own delay
        // while walking, hold the first frame otherwise.
        let action_index = ro_sprite.direction as usize;
        let Some(action_data) = animation.actions.get(action_index) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        if action_data.frames.is_empty() {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }

        let frame_index = if ro_sprite.action_type == ActionType::Walk {
            let delay = action_data.delay_ms.max(1.0);
            (game_time_ms as f32 / delay) as usize % action_data.frames.len()
        } else {
            0
        };

        let Some(frame) = action_data.frames.get(frame_index) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        let Some(part) = frame.parts.get(cart.part) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        if let Some(texture) = animation.textures.get(part.texture_index) {
            set_layer_texture(&mut materials, &material_handle.0, texture);
        }

        let mut scale_x = part.scale.x * part.texture_size.x * SPRITE_WORLD_SCALE;
        let scale_y = part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE;

        if part.mirror {
            scale_x = -scale_x;
        }

        let behind = cart_behind_offset(ro_sprite.direction) * CART_BACK_DISTANCE;
        let part_z = layer_order(LAYER_CART) as f32 * Z_OFFSET_PER_LAYER + cart.part as f32 * 0.001;

        let current = *transform;
        transform.set_if_neq(Transform {
            scale: Vec3::new(scale_x, scale_y, 1.0),
            translation: Vec3::new(
                part.position.x * SPRITE_WORLD_SCALE + behind.x,
                part.position.y * SPRITE_WORLD_SCALE,
                part_z + behind.y,
            ),
            ..current
        });

        visibility.set_if_neq(Visibility::Inherited);
    }
}
