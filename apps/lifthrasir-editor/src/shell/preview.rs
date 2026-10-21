//! Previews the browser selection: models render through the runtime's own
//! prop spawning on a separate render layer into an image, ground textures
//! show as images, sounds play once on request.

use super::browser::{AssetRef, BrowserSelection};
use crate::EditorOnly;
use bevy::{
    app::{HierarchyPropagatePlugin, Propagate},
    camera::{RenderTarget, primitives::Aabb, visibility::RenderLayers},
    prelude::*,
    render::render_resource::TextureFormat,
    transform::TransformSystems,
};
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use bevy_kira_audio::{AudioChannel, AudioControl, AudioSource};
use lifthrasir_data::lif::LifProp;
use lifthrasir_runtime::{
    audio::SfxChannel,
    rendering::EquipmentPreviewCamera,
    world::{LifPropRef, PropSceneContext},
};
use std::f32::consts::FRAC_PI_8;

/// The render layer only the preview camera and preview models live on.
const PREVIEW_LAYER: usize = 1;
/// Far from any map, so preview models never meet map geometry.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, -10_000.0, 0.0);
const PREVIEW_SIZE: u32 = 256;
const TURNTABLE_SPEED: f32 = 0.4;
/// Default map exposure; the preview light is tuned to it.
const PREVIEW_EXPOSURE_EV100: f32 = 10.5;
const PREVIEW_SUN_LUX: f32 = 8_000.0;

/// The preview's render target, the selected ground texture and a pending
/// Play request.
#[derive(Resource)]
pub struct PreviewState {
    image: Handle<Image>,
    ground: Option<Handle<Image>>,
    pub play: bool,
}

impl PreviewState {
    /// egui ids for the model render target and the selected ground texture.
    pub fn texture_ids(&self, contexts: &mut EguiContexts) -> PreviewTextures {
        PreviewTextures {
            model: contexts.add_image(EguiTextureHandle::Weak(self.image.id())),
            ground: self
                .ground
                .as_ref()
                .map(|ground| contexts.add_image(EguiTextureHandle::Weak(ground.id()))),
        }
    }
}

pub struct PreviewTextures {
    model: egui::TextureId,
    ground: Option<egui::TextureId>,
}

#[derive(Component)]
struct PreviewRoot;

#[derive(Component)]
struct PreviewCamera;

/// Preview camera, layer propagation and the preview root lifecycle.
pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HierarchyPropagatePlugin::<RenderLayers>::new(PostUpdate))
            .add_systems(Startup, spawn_preview_camera)
            .add_systems(
                Update,
                (
                    swap_preview.run_if(resource_changed::<BrowserSelection>),
                    turntable,
                    play_preview_sound.run_if(|state: Res<PreviewState>| state.play),
                ),
            )
            .add_systems(PostUpdate, frame_preview.after(TransformSystems::Propagate));
    }
}

fn spawn_preview_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_target_texture(
        PREVIEW_SIZE,
        PREVIEW_SIZE,
        TextureFormat::Bgra8UnormSrgb,
        None,
    ));
    let layer = RenderLayers::layer(PREVIEW_LAYER);
    commands.spawn((
        Name::new("preview camera"),
        PreviewCamera,
        // The runtime's "not the world camera" marker: its billboard and
        // effect-ordering systems resolve the world camera with `single()`.
        EquipmentPreviewCamera,
        EditorOnly,
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.12, 0.14)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        bevy::camera::Exposure {
            ev100: PREVIEW_EXPOSURE_EV100,
        },
        Projection::Perspective(PerspectiveProjection {
            far: 10_000.0,
            ..default()
        }),
        Transform::from_translation(PREVIEW_ORIGIN + Vec3::new(0.0, 20.0, 50.0))
            .looking_at(PREVIEW_ORIGIN, Vec3::Y),
        layer.clone(),
    ));
    commands.spawn((
        Name::new("preview light"),
        EditorOnly,
        DirectionalLight {
            illuminance: PREVIEW_SUN_LUX,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-1.0, -2.0, -1.0), Vec3::Y),
        layer,
    ));
    commands.insert_resource(PreviewState {
        image,
        ground: None,
        play: false,
    });
}

/// Replaces the preview with the new selection: a model gets a preview root
/// the runtime turns into its prop scene; a ground texture starts loading.
fn swap_preview(
    mut commands: Commands,
    selection: Res<BrowserSelection>,
    roots: Query<Entity, With<PreviewRoot>>,
    mut cameras: Query<&mut Camera, With<PreviewCamera>>,
    mut state: ResMut<PreviewState>,
    assets: Res<AssetServer>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    state.ground = match &selection.0 {
        Some(AssetRef::GroundTexture(uri)) => Some(assets.load(uri.clone())),
        _ => None,
    };
    let model = match &selection.0 {
        Some(AssetRef::Model(uri)) => Some(uri.clone()),
        _ => None,
    };
    for mut camera in &mut cameras {
        camera.is_active = model.is_some();
    }
    let Some(model) = model else {
        return;
    };
    commands.spawn((
        Name::new("asset preview"),
        PreviewRoot,
        EditorOnly,
        LifPropRef(LifProp {
            model,
            anim_type: 0,
            anim_speed: 1.0,
        }),
        PropSceneContext::default(),
        Propagate(RenderLayers::layer(PREVIEW_LAYER)),
        Transform::from_translation(PREVIEW_ORIGIN),
        Visibility::default(),
    ));
}

fn turntable(time: Res<Time>, mut roots: Query<&mut Transform, With<PreviewRoot>>) {
    for mut transform in &mut roots {
        transform.rotate_y(TURNTABLE_SPEED * time.delta_secs());
    }
}

/// Points the preview camera at the loaded model. Bounds are taken in the
/// root's frame and widened by the centre's distance from the turntable axis,
/// so the framing does not wobble while the model spins.
fn frame_preview(
    roots: Query<(Entity, &GlobalTransform), With<PreviewRoot>>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut cameras: Query<&mut Transform, With<PreviewCamera>>,
) {
    let Ok((root, root_global)) = roots.single() else {
        return;
    };
    let to_root = root_global.affine().inverse();
    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    for (aabb, global) in children
        .iter_descendants(root)
        .filter_map(|entity| bounds.get(entity).ok())
    {
        let to_root = to_root * global.affine();
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let point = to_root
                .transform_point3(Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * sign);
            min = min.min(point);
            max = max.max(point);
        }
    }
    if min.x > max.x {
        return;
    }
    let centre = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5 + centre.xz().length();
    let target = root_global.translation() + Vec3::Y * centre.y;
    let distance = radius.max(1.0) / FRAC_PI_8.sin();
    for mut transform in &mut cameras {
        *transform =
            Transform::from_translation(target + Vec3::new(0.0, 0.4, 1.0).normalize() * distance)
                .looking_at(target, Vec3::Y);
    }
}

fn play_preview_sound(
    mut state: ResMut<PreviewState>,
    selection: Res<BrowserSelection>,
    sfx: Option<Res<AudioChannel<SfxChannel>>>,
    assets: Res<AssetServer>,
) {
    state.play = false;
    let Some(AssetRef::Sound(uri)) = &selection.0 else {
        return;
    };
    let Some(sfx) = sfx else {
        error!("cannot preview '{uri}': the SFX audio channel is missing");
        return;
    };
    sfx.play(assets.load::<AudioSource>(uri.clone()));
}

/// The preview area under the browser list.
pub fn preview_ui(
    ui: &mut egui::Ui,
    selection: &BrowserSelection,
    state: &mut PreviewState,
    textures: &PreviewTextures,
) {
    let size = egui::vec2(PREVIEW_SIZE as f32, PREVIEW_SIZE as f32);
    match &selection.0 {
        Some(AssetRef::Model(_)) => {
            ui.image(egui::load::SizedTexture::new(textures.model, size));
        }
        Some(AssetRef::GroundTexture(_)) => {
            if let Some(ground) = textures.ground {
                ui.image(egui::load::SizedTexture::new(ground, size));
            }
        }
        Some(AssetRef::Sound(_)) => {
            state.play |= ui.button("Play").clicked();
        }
        Some(AssetRef::Effect(_)) | None => {}
    }
}
