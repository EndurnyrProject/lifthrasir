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
use std::{
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_8,
};

/// The render layer only the preview camera and preview models live on.
const PREVIEW_LAYER: usize = 1;
/// Far from any map, so preview models never meet map geometry.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, -10_000.0, 0.0);
const PREVIEW_SIZE: u32 = 256;
/// The square render target used by one model tile in the asset grid.
pub const GRID_THUMBNAIL_SIZE: u32 = 96;
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

/// Images used by the asset browser's grid. Models need a dedicated render
/// target; ground textures can be shown directly from the asset server.
#[derive(Resource, Default)]
pub struct GridThumbnails {
    active: bool,
    models: HashMap<String, ModelThumbnail>,
    ground: HashMap<String, Handle<Image>>,
    pending_models: Vec<String>,
    visible_models: HashSet<String>,
    visible_ground: HashSet<String>,
    free_slots: Vec<ThumbnailSlot>,
    next_model_origin: u32,
}

struct ModelThumbnail {
    image: Handle<Image>,
    root: Option<Entity>,
    camera: Option<Entity>,
    origin: Vec3,
}

struct ThumbnailSlot {
    image: Handle<Image>,
    camera: Option<Entity>,
    origin: Vec3,
}

impl GridThumbnails {
    /// Clears the previous UI frame's visible set. Grid tiles repopulate it as
    /// they are drawn, so off-screen model renderers can be reclaimed.
    pub fn begin_frame(&mut self) {
        self.active = false;
        self.visible_models.clear();
        self.visible_ground.clear();
    }

    /// Returns the texture id for a model tile, scheduling its scene and
    /// camera the first time it becomes visible in the grid.
    pub fn model_texture_id(
        &mut self,
        uri: &str,
        contexts: &mut EguiContexts,
        images: &mut Assets<Image>,
    ) -> egui::TextureId {
        self.active = true;
        self.visible_models.insert(uri.to_owned());
        if !self.models.contains_key(uri) {
            let slot = self.free_slots.pop().unwrap_or_else(|| {
                self.next_model_origin += 1;
                ThumbnailSlot {
                    image: images.add(Image::new_target_texture(
                        GRID_THUMBNAIL_SIZE,
                        GRID_THUMBNAIL_SIZE,
                        TextureFormat::Bgra8UnormSrgb,
                        None,
                    )),
                    camera: None,
                    origin: PREVIEW_ORIGIN + Vec3::X * self.next_model_origin as f32 * 10_000.0,
                }
            });
            self.models.insert(
                uri.to_owned(),
                ModelThumbnail {
                    image: slot.image,
                    root: None,
                    camera: slot.camera,
                    origin: slot.origin,
                },
            );
            self.pending_models.push(uri.to_owned());
        }
        contexts.add_image(EguiTextureHandle::Weak(self.models[uri].image.id()))
    }

    /// Returns the texture id for a ground-texture tile and retains its handle
    /// so Bevy does not unload it while egui only has a weak texture id.
    pub fn ground_texture_id(
        &mut self,
        uri: &str,
        contexts: &mut EguiContexts,
        assets: &AssetServer,
    ) -> egui::TextureId {
        self.active = true;
        self.visible_ground.insert(uri.to_owned());
        let image = self
            .ground
            .entry(uri.to_owned())
            .or_insert_with(|| assets.load::<Image>(uri.to_owned()));
        contexts.add_image(EguiTextureHandle::Weak(image.id()))
    }
}

#[derive(Component)]
struct PreviewRoot;

#[derive(Component)]
struct PreviewCamera;

#[derive(Component)]
struct GridThumbnailRoot {
    camera: Entity,
}

#[derive(Component)]
struct GridThumbnailCamera;

/// Preview camera, layer propagation and the preview root lifecycle.
pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HierarchyPropagatePlugin::<RenderLayers>::new(PostUpdate))
            .add_systems(Startup, spawn_preview_camera)
            .init_resource::<GridThumbnails>()
            .add_systems(
                Update,
                (
                    swap_preview.run_if(resource_changed::<BrowserSelection>),
                    spawn_grid_thumbnails,
                    turntable,
                    play_preview_sound.run_if(|state: Res<PreviewState>| state.play),
                ),
            )
            .add_systems(
                PostUpdate,
                (frame_preview, frame_grid_thumbnails).after(TransformSystems::Propagate),
            );
    }
}

/// Spawns each requested model at a separate distant origin. All thumbnail
/// cameras share the preview layer, but their narrow views cannot see another
/// tile's model.
fn spawn_grid_thumbnails(
    mut commands: Commands,
    mut thumbnails: ResMut<GridThumbnails>,
    mut cameras: Query<&mut Camera, With<GridThumbnailCamera>>,
) {
    thumbnails.release_invisible(&mut commands, &mut cameras);
    let mut started = 0;
    for model in std::mem::take(&mut thumbnails.pending_models) {
        if !thumbnails.visible_models.contains(&model) {
            continue;
        }
        let Some(thumbnail) = thumbnails.models.get(&model) else {
            continue;
        };
        // Avoid launching an entire viewport of GLB loads on a scrollbar jump.
        if started >= 2 {
            thumbnails.pending_models.push(model);
            continue;
        }
        started += 1;
        let image = thumbnail.image.clone();
        let origin = thumbnail.origin;
        let camera = if let Some(camera) = thumbnail.camera {
            if let Ok(mut state) = cameras.get_mut(camera) {
                state.is_active = true;
            }
            camera
        } else {
            commands
                .spawn((
                    Name::new(format!("thumbnail camera {model}")),
                    GridThumbnailCamera,
                    EquipmentPreviewCamera,
                    EditorOnly,
                    Camera3d::default(),
                    Camera {
                        order: -2,
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.12, 0.14)),
                        ..default()
                    },
                    RenderTarget::Image(image.into()),
                    bevy::camera::Exposure {
                        ev100: PREVIEW_EXPOSURE_EV100,
                    },
                    Projection::Perspective(PerspectiveProjection {
                        far: 10_000.0,
                        ..default()
                    }),
                    Transform::from_translation(origin + Vec3::new(0.0, 20.0, 50.0))
                        .looking_at(origin, Vec3::Y),
                    RenderLayers::layer(PREVIEW_LAYER),
                ))
                .id()
        };
        let root = commands
            .spawn((
                Name::new(format!("thumbnail model {model}")),
                GridThumbnailRoot { camera },
                EditorOnly,
                LifPropRef(LifProp {
                    model: model.clone(),
                    anim_type: 0,
                    anim_speed: 1.0,
                }),
                PropSceneContext::default(),
                Propagate(RenderLayers::layer(PREVIEW_LAYER)),
                Transform::from_translation(origin),
                Visibility::default(),
            ))
            .id();
        if let Some(thumbnail) = thumbnails.models.get_mut(&model) {
            thumbnail.root = Some(root);
            thumbnail.camera = Some(camera);
        }
    }
}

impl GridThumbnails {
    fn release_invisible(
        &mut self,
        commands: &mut Commands,
        cameras: &mut Query<&mut Camera, With<GridThumbnailCamera>>,
    ) {
        let expired: Vec<String> = self
            .models
            .keys()
            .filter(|model| !self.visible_models.contains(*model))
            .cloned()
            .collect();
        for model in expired {
            let thumbnail = self.models.remove(&model).expect("indexed thumbnail");
            if let Some(root) = thumbnail.root {
                commands.entity(root).despawn();
            }
            if let Some(camera) = thumbnail.camera
                && let Ok(mut state) = cameras.get_mut(camera)
            {
                state.is_active = false;
            }
            self.free_slots.push(ThumbnailSlot {
                image: thumbnail.image,
                camera: thumbnail.camera,
                origin: thumbnail.origin,
            });
        }
        self.ground
            .retain(|uri, _| self.visible_ground.contains(uri));
        if !self.active {
            // Keep slots reusable while browsing, but do not retain cameras
            // and strong render-target handles after leaving the grid.
            for slot in self.free_slots.drain(..) {
                if let Some(camera) = slot.camera {
                    commands.entity(camera).despawn();
                }
            }
            self.pending_models.clear();
            self.next_model_origin = 0;
        }
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

fn frame_grid_thumbnails(
    roots: Query<(Entity, &GridThumbnailRoot, &GlobalTransform)>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut cameras: Query<&mut Transform, With<GridThumbnailCamera>>,
) {
    for (root, thumbnail, root_global) in &roots {
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
                let point = to_root.transform_point3(
                    Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * sign,
                );
                min = min.min(point);
                max = max.max(point);
            }
        }
        if min.x > max.x {
            continue;
        }
        let centre = (min + max) * 0.5;
        let radius = (max - min).length() * 0.5 + centre.xz().length();
        let target = root_global.translation() + Vec3::Y * centre.y;
        let distance = radius.max(1.0) / FRAC_PI_8.sin();
        let Ok(mut camera) = cameras.get_mut(thumbnail.camera) else {
            continue;
        };
        *camera =
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

#[cfg(test)]
mod thumbnail_tests {
    use super::*;

    #[test]
    fn offscreen_models_release_scene_but_recycle_camera_and_target() {
        let mut app = App::new();
        let root = app.world_mut().spawn_empty().id();
        let child = app.world_mut().spawn(ChildOf(root)).id();
        let camera = app
            .world_mut()
            .spawn((GridThumbnailCamera, Camera::default()))
            .id();
        let image = Handle::<Image>::default();
        let mut thumbnails = GridThumbnails {
            active: true,
            ..default()
        };
        thumbnails.models.insert(
            "model.glb".into(),
            ModelThumbnail {
                image: image.clone(),
                root: Some(root),
                camera: Some(camera),
                origin: PREVIEW_ORIGIN,
            },
        );
        thumbnails
            .ground
            .insert("ground.ktx2".into(), image.clone());
        app.insert_resource(thumbnails)
            .add_systems(Update, spawn_grid_thumbnails);
        app.update();

        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(child).is_err());
        assert!(!app.world().get::<Camera>(camera).unwrap().is_active);
        let thumbnails = app.world().resource::<GridThumbnails>();
        assert!(thumbnails.models.is_empty());
        assert!(thumbnails.ground.is_empty());
        assert_eq!(thumbnails.free_slots.len(), 1);
        assert_eq!(thumbnails.free_slots[0].camera, Some(camera));
        assert_eq!(thumbnails.free_slots[0].image, image);
    }

    #[test]
    fn visible_thumbnails_are_retained_and_frame_reset_clears_visibility() {
        let mut app = App::new();
        let selected_preview = app.world_mut().spawn(PreviewRoot).id();
        let selected_camera = app.world_mut().spawn(PreviewCamera).id();
        let pooled_camera = app.world_mut().spawn(GridThumbnailCamera).id();
        let root = app.world_mut().spawn_empty().id();
        let camera = app
            .world_mut()
            .spawn((GridThumbnailCamera, Camera::default()))
            .id();
        let mut thumbnails = GridThumbnails {
            active: true,
            ..default()
        };
        thumbnails.free_slots.push(ThumbnailSlot {
            image: Handle::default(),
            camera: Some(pooled_camera),
            origin: PREVIEW_ORIGIN,
        });
        thumbnails.models.insert(
            "model.glb".into(),
            ModelThumbnail {
                image: Handle::default(),
                root: Some(root),
                camera: Some(camera),
                origin: PREVIEW_ORIGIN,
            },
        );
        thumbnails.visible_models.insert("model.glb".into());
        thumbnails.visible_ground.insert("ground.ktx2".into());
        thumbnails
            .ground
            .insert("ground.ktx2".into(), Handle::default());
        app.insert_resource(thumbnails)
            .add_systems(Update, spawn_grid_thumbnails);
        app.update();
        assert!(app.world().get_entity(root).is_ok());
        assert_eq!(app.world().resource::<GridThumbnails>().free_slots.len(), 1);
        app.world_mut()
            .resource_mut::<GridThumbnails>()
            .begin_frame();
        app.update();
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(camera).is_err());
        assert!(app.world().get_entity(pooled_camera).is_err());
        assert!(app.world().get_entity(selected_preview).is_ok());
        assert!(app.world().get_entity(selected_camera).is_ok());
        let thumbnails = app.world().resource::<GridThumbnails>();
        assert!(thumbnails.ground.is_empty());
        assert!(thumbnails.models.is_empty());
        assert!(thumbnails.free_slots.is_empty());
        assert!(thumbnails.pending_models.is_empty());
        assert_eq!(thumbnails.next_model_origin, 0);
    }
}
