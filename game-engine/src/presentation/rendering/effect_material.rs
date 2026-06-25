use crate::infrastructure::effect::EffectBlend;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// Minimal unlit material for STR skill-effect billboards. The fragment shader
/// outputs `texture x vertex_color`; blend is driven entirely by `alpha_mode`,
/// which Bevy already specializes the pipeline on. Per-frame texture is set on
/// the material; per-frame geometry / UV / tint live in the mesh attributes.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct EffectMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub base_color_texture: Handle<Image>,
    pub alpha_mode: AlphaMode,
}

impl Material for EffectMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/effect.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Effect billboards are 2D quads seen from either side, and both the
        // per-frame Y-flip and the STR layer rotation flip triangle winding.
        // Disable back-face culling (as the sprite material does) so the whole
        // quad always renders; otherwise the back-facing half of each effect
        // sprite is culled — e.g. the magnus angel's head.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Map a decoded STR `EffectBlend` onto the Bevy `AlphaMode` the renderer uses.
pub fn alpha_mode_for(blend: EffectBlend) -> AlphaMode {
    match blend {
        EffectBlend::Add => AlphaMode::Add,
        EffectBlend::Blend => AlphaMode::Blend,
        EffectBlend::Multiply => AlphaMode::Multiply,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_mode_for_maps_each_blend() {
        assert_eq!(alpha_mode_for(EffectBlend::Add), AlphaMode::Add);
        assert_eq!(alpha_mode_for(EffectBlend::Blend), AlphaMode::Blend);
        assert_eq!(alpha_mode_for(EffectBlend::Multiply), AlphaMode::Multiply);
    }
}
