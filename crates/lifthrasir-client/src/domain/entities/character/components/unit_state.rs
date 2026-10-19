use bevy::prelude::*;

/// Legacy RO unit-state flags carried by aesir's `UnitStateChange` packet.
///
/// This is the older `opt1`/`opt2`/`option`/`opt3` channel, distinct from the
/// modern EFST `StatusEffects` channel. All four fields are stored verbatim so
/// the render subset can be recomputed deterministically; only body poses and
/// hide/cloak visibility are rendered today (see `apply_unit_state`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnitState {
    /// `opt1` — single-valued sprite body state (stone/freeze/stun/sleep/...).
    pub body_state: u32,
    /// `opt2` — health-state bitmask (poison/curse/silence/...).
    pub health_state: u32,
    /// `option` — effect-state bitmask (hide/cloak/mount/cart/...).
    pub effect_state: u32,
    /// `opt3` — virtue/karma marker.
    pub virtue: u32,
}
