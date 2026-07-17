use super::impact::{drive_factor, FactorMaterial};
use super::VfxSystems;
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
        };
        material.set_factor(0.6);
        assert_eq!(material.params.factor, 0.6);
    }
}
