use bevy::prelude::*;

/// Domain→presentation seam for procedural (non-STR) skill visuals (design §4).
/// The trigger resolves the anchor's world position and tint, keeping
/// `domain/effects` free of `Material`/`bevy_hanabi` types; the presentation VFX
/// plugin consumes this and builds the material tree.
#[derive(Message)]
pub struct PlayProceduralVfx {
    /// Procedural effect key, e.g. `"bash"`.
    pub key: String,
    /// World position, already resolved by the trigger.
    pub position: Vec3,
    /// Descriptor tint.
    pub color: Color,
}
