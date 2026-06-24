pub mod components;
pub mod systems;

pub use components::{ActiveEffect, EffectAnchor, EffectFrameTimer, EffectLayer, EffectLifetime};
pub use systems::{
    advance_effect_timers, despawn_finished_effects, follow_effect_anchor,
    initialize_effect_layers, interpolate_layer_frame, rebuild_effect_layers, spawn_effect,
    RenderFrame, STR_WORLD_SCALE,
};
