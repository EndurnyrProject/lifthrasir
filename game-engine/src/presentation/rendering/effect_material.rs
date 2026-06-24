use crate::infrastructure::effect::EffectBlend;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
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
