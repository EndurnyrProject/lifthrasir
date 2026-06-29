use super::VfxSystems;
use crate::domain::entities::markers::WarpPortal;
use crate::utils::constants::CELL_SIZE;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// Portal VFX anchor. Spawn with a `Transform`; on the next frame the plugin
/// attaches the swirl surface (floor-laid quad + `PortalMaterial`), parameterised
/// by colour and radius.
#[derive(Component, Clone, Copy)]
pub struct PortalVfx {
    pub color: Color,
    /// Ring radius in world units.
    pub radius: f32,
}

impl Default for PortalVfx {
    fn default() -> Self {
        Self {
            color: Color::srgb(0.3, 0.8, 1.0),
            radius: 1.0,
        }
    }
}

/// Registers the portal `MaterialPlugin` and the attach system. `HanabiPlugin`
/// is owned by the parent `VfxPlugin`, not here.
pub struct PortalVfxPlugin;

impl Plugin for PortalVfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PortalMaterial>::default())
            .init_resource::<PortalAssets>()
            .add_systems(
                Update,
                (attach_warp_portal_vfx, attach_portal_visuals)
                    .chain()
                    .in_set(VfxSystems),
            );
    }
}

/// Shared portal noise, loaded once with a Repeat sampler so the polar swirl
/// tiles seamlessly. Every portal samples the same texture.
#[derive(Resource)]
struct PortalAssets {
    noise: Handle<Image>,
}

impl FromWorld for PortalAssets {
    fn from_world(world: &mut World) -> Self {
        let noise = world
            .resource::<AssetServer>()
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| {
                s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    address_mode_w: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
            })
            .load("ro://effects/portal_noise.png");
        Self { noise }
    }
}

/// Bridge: a warp-portal NPC (domain `WarpPortal`) gets a `PortalVfx` so the
/// shared attach path below builds its surface + sparkles.
fn attach_warp_portal_vfx(mut commands: Commands, warps: Query<Entity, Added<WarpPortal>>) {
    for entity in &warps {
        commands.entity(entity).insert(PortalVfx {
            color: Color::srgb(0.3, 0.8, 1.0),
            radius: CELL_SIZE,
        });
    }
}

/// On `PortalVfx` spawn, attach the swirl surface (floor-laid quad +
/// `PortalMaterial`) to the same entity.
fn attach_portal_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    assets: Res<PortalAssets>,
    portals: Query<(Entity, &PortalVfx), Added<PortalVfx>>,
) {
    for (entity, portal) in &portals {
        // Floor-laid quad (XZ plane, facing +Y), lifted a hair off the ground to
        // dodge z-fighting with the terrain.
        let mesh = meshes.add(
            Mesh::from(Plane3d::new(Vec3::Y, Vec2::splat(portal.radius)))
                .translated_by(Vec3::Y * portal.radius * 0.05),
        );
        let material = materials.add(portal_material(portal.color, assets.noise.clone()));
        commands
            .entity(entity)
            .insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

/// Unlit swirl-tunnel material for the portal surface. See
/// `assets/data/effects/portal.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PortalMaterial {
    #[uniform(0)]
    pub params: PortalParams,
    #[texture(1)]
    #[sampler(2)]
    pub noise: Handle<Image>,
}

/// Packed portal shader parameters. Field order and types must match the
/// `PortalParams` struct in `portal.wgsl`.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct PortalParams {
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    /// x=open_amount y=density z=edge_softness w=emission_strength
    pub shape: Vec4,
    /// x=depth_amount y=shrink_amount z=fade_amount w=layers
    pub depth: Vec4,
    /// x=speed_scale y=spin z=inward w=base_motion
    pub motion: Vec4,
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "ro://effects/portal.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The RO camera can tilt past the quad's plane; render both faces.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Build a portal surface material from a single colour. Secondary (deep) colour
/// is a dimmed primary; tuning knobs are BinbunVFX's portal-scene defaults.
///
/// NOTE: tuning constants are baked here. Move to a RON asset if portals ever
/// need per-map variation.
fn portal_material(color: Color, noise: Handle<Image>) -> PortalMaterial {
    let c = color.to_linear();
    let primary = Vec4::new(c.red, c.green, c.blue, 1.0);
    PortalMaterial {
        params: PortalParams {
            primary_color: primary,
            secondary_color: Vec4::new(c.red * 0.2, c.green * 0.2, c.blue * 0.2, 1.0),
            shape: Vec4::new(1.0, 0.4, 0.5, 2.0),
            depth: Vec4::new(1.0, 0.5, 1.0, 8.0),
            motion: Vec4::new(1.0, 0.1, 0.2, 0.2),
        },
        noise,
    }
}
