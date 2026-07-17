use super::impact::{FactorMaterial, FactorRamp, ImpactAssets, LightFade, LIGHT_PEAK};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy_hanabi::ParticleEffect;

/// Unlit additive electric-detonation material for Jupitel Thunder: flickering
/// core, crackling procedural bolts, expanding shock ring, all in one billboard
/// fragment. Animated by `FactorRamp` via `factor`; crackle jitter comes from
/// the shader-side globals clock. See `assets/data/effects/jupitel_thunder.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct JupitelMaterial {
    #[uniform(0)]
    pub params: JupitelParams,
}

/// Packed Jupitel parameters. Field order and types must match the
/// `JupitelParams` struct in `jupitel_thunder.wgsl`.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct JupitelParams {
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    /// x=emission y=bolt_count z=crackle_hz
    pub shape: Vec4,
    pub factor: f32,
}

impl Material for JupitelMaterial {
    fn vertex_shader() -> ShaderRef {
        "ro://effects/jupitel_thunder.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "ro://effects/jupitel_thunder.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

impl FactorMaterial for JupitelMaterial {
    fn set_factor(&mut self, factor: f32) {
        self.params.factor = factor;
    }
}

/// Spawn the Jupitel Thunder detonation: a `FactorRamp` parent at `position`
/// carrying the electric billboard, a strong blue point-light pop, and the
/// blue-white hanabi spark burst. The ramp outlives the sparks' max lifetime,
/// then despawns the tree.
pub fn spawn_jupitel_burst(
    commands: &mut Commands,
    materials: &mut Assets<JupitelMaterial>,
    assets: &ImpactAssets,
    position: Vec3,
) {
    let material = materials.add(JupitelMaterial {
        params: JupitelParams {
            primary_color: Vec4::new(3.5, 4.0, 6.0, 1.0),
            secondary_color: Vec4::new(0.25, 0.55, 3.2, 1.0),
            shape: Vec4::new(2.0, 7.0, 24.0, 0.0),
            factor: 0.0,
        },
    });
    let sparks = assets.bursts["jupitel_thunder"].0.clone();
    let light_color = Color::srgb(0.55, 0.65, 1.0);
    let light_peak = 2.0 * LIGHT_PEAK;

    commands
        .spawn((
            FactorRamp::new(0.7),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(26.0)),
            ));
            parent.spawn((
                PointLight {
                    color: light_color,
                    intensity: light_peak,
                    range: 45.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                LightFade::new(0.22, light_peak),
            ));
            parent.spawn(ParticleEffect::new(sparks));
        });
}
