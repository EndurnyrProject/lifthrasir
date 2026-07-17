use super::impact::{
    drive_factor, spark_garnish_bundle, FactorMaterial, FactorRamp, ImpactAssets, LightFade,
    LIGHT_PEAK,
};
use super::VfxSystems;
use crate::infrastructure::effect::ShaderFxEntry;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// Generic unlit additive billboard material for procedural skill effects. The
/// `kind` uniform selects the per-skill fragment function in the uber-shader, so
/// a new effect is one WGSL fragment plus one `shader_fx.ron` entry, zero Rust.
/// Animated by `FactorRamp` via `factor`. See `assets/data/effects/skill_fx.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SkillFxMaterial {
    #[uniform(0)]
    pub params: SkillFxParams,
    /// Optional classic GRF effect texture the fragment may sample (kinds that
    /// don't sample leave it `None`, which binds Bevy's fallback image). Loaded
    /// from `ShaderFxEntry::texture` in `spawn_shader_fx`.
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

/// Packed skill-fx parameters. Field order and types must match the
/// `SkillFxParams` struct in `skill_fx.wgsl`.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct SkillFxParams {
    pub kind: u32,
    pub primary: Vec4,
    pub secondary: Vec4,
    /// Per-kind scalars; meaning documented in each fragment function header.
    pub shape: Vec4,
    pub factor: f32,
}

impl Material for SkillFxMaterial {
    fn vertex_shader() -> ShaderRef {
        "ro://effects/skill_fx.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "ro://effects/skill_fx.wgsl".into()
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

impl FactorMaterial for SkillFxMaterial {
    fn set_factor(&mut self, factor: f32) {
        self.params.factor = factor;
    }
}

/// Spawn a data-driven shader effect from a `ShaderFxCatalog` entry: a
/// `FactorRamp` parent at `position` carrying a billboard quad with a
/// `SkillFxMaterial` built from the entry, plus an optional point-light pop and
/// an optional tintable spark garnish when the entry declares them. Generalizes
/// `spawn_jupitel_burst`; the light `range` (45.0) matches jupitel's, the only
/// light knob the entry does not carry.
pub fn spawn_shader_fx(
    commands: &mut Commands,
    materials: &mut Assets<SkillFxMaterial>,
    asset_server: &AssetServer,
    assets: &ImpactAssets,
    entry: &ShaderFxEntry,
    position: Vec3,
) {
    let texture = entry
        .texture
        .as_ref()
        .map(|path| asset_server.load(format!("ro://{path}")));

    let material = materials.add(SkillFxMaterial {
        params: SkillFxParams {
            kind: entry.kind,
            primary: entry.primary.into(),
            secondary: entry.secondary.into(),
            shape: entry.shape.into(),
            factor: 0.0,
        },
        texture,
    });

    commands
        .spawn((
            FactorRamp::new(entry.duration),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(entry.scale)),
            ));
            if let Some(light) = &entry.light {
                let peak = light.intensity_scale * LIGHT_PEAK;
                parent.spawn((
                    PointLight {
                        color: Color::srgb(light.color.0, light.color.1, light.color.2),
                        intensity: peak,
                        range: 45.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    LightFade::new(light.fade, peak),
                ));
            }
            if let Some(garnish) = &entry.garnish {
                parent.spawn(spark_garnish_bundle(assets, garnish.tint.into()));
            }
        });
}

/// Registers the `SkillFxMaterial` `MaterialPlugin` and its factor driver.
/// `HanabiPlugin` is owned by the parent `VfxPlugin`, not here.
pub struct SkillFxPlugin;

impl Plugin for SkillFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkillFxMaterial>::default())
            .add_systems(Update, drive_factor::<SkillFxMaterial>.in_set(VfxSystems));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_factor_writes_factor_uniform() {
        let mut material = SkillFxMaterial {
            params: SkillFxParams {
                kind: 0,
                primary: Vec4::ONE,
                secondary: Vec4::ONE,
                shape: Vec4::ZERO,
                factor: 0.0,
            },
            texture: None,
        };
        material.set_factor(0.6);
        assert_eq!(material.params.factor, 0.6);
    }
}
