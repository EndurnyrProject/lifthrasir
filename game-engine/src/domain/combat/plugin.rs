use crate::app::combat_plugin::CombatDomainPlugin;
use bevy::prelude::*;

/// Combat Plugin
///
/// Handles all combat-related functionality including:
/// - Processing combat actions from server (ZC_NOTIFY_ACT)
/// - Attack animations with ASPD-based speed
/// - Hit reactions and flinch animations
/// - Death animations
/// - Damage number display
///
/// # System Flow
///
/// 1. `process_combat_actions` - Receives ZC_NOTIFY_ACT, starts attack/hit animations
/// 2. `handle_hit_reactions` - Plays flinch animation if entity has no Endure
/// 3. `update_attack_timers` - Updates attack animation timers
/// 4. `update_hit_stun` - Updates hit stun timers
/// 5. `handle_death` - Plays death animation when entity vanishes with type 1
///
/// Floating damage numbers are rendered by the UI layer (`DamageNumberPlugin`),
/// which consumes the `DisplayDamageNumber` messages these systems emit.
///
/// # Integration
///
/// ```ignore
/// app.add_plugins(CombatPlugin);
/// ```
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Register network protocol messages
        app.add_message::<super::events::DisplayDamageNumber>();

        // Add combat domain plugin (auto-plugin with systems)
        app.add_plugins(CombatDomainPlugin);

        debug!("CombatPlugin initialized");
    }
}
