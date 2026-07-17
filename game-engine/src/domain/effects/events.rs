use bevy::prelude::*;

/// Domain→presentation seam for procedural (non-STR) skill visuals (design §4).
/// The trigger resolves the anchor's world position and tint, keeping
/// `domain/effects` free of `Material`/`bevy_hanabi` types; the presentation VFX
/// plugin consumes this and builds the material tree.
#[derive(Message)]
pub struct PlayProceduralVfx {
    /// Procedural effect key, e.g. `"bash"`.
    pub key: String,
    /// World position, already resolved by the trigger. For a traveling effect
    /// this is the target the projectile flies to.
    pub position: Vec3,
    /// Caster world position, when known. A catalog entry with `travel` and a
    /// `Some` source flies its projectile from here to `position`; `None` (or a
    /// non-traveling entry) plays the effect straight at `position`.
    pub source: Option<Vec3>,
    /// Number of hits the skill dealt (bolt count). A `per_hit` travel entry
    /// launches this many staggered projectiles, one per bolt; other entries
    /// ignore it. Always at least 1.
    pub hits: u32,
    /// Skill sound to play per projectile launch, for `travel` entries. `None`
    /// for non-travel effects (their sound plays once at cast in the trigger).
    /// Set only when the effect travels, so each orb whooshes as it fires.
    pub sound: Option<String>,
    /// Descriptor tint.
    pub color: Color,
}
