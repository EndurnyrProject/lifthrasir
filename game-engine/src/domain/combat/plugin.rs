use crate::app::combat_plugin::CombatDomainPlugin;
use bevy::prelude::*;

/// Combat Plugin
///
/// Handles all combat-related functionality including:
/// - Processing combat actions from the server
/// - Attack animations with server-provided motion timing
/// - Hit reactions and flinch animations
/// - Death animations
/// - Damage number display
///
/// # System Flow
///
/// 1. `process_combat_actions` - Interprets `DamageReceived` messages
/// 2. `apply_pending_hit_reactions` - Displays damage and starts timed flinches
/// 3. `handle_hit_reactions` - Adds fallback timing to otherwise untimed hit states
/// 4. `update_attack_timers` - Updates attack animation timers
/// 5. `update_hit_stun` - Updates hit stun timers
/// 6. `handle_death` - Plays death animation when an entity dies
/// 7. `detect_local_death` - Marks the local player dead when its applied HP reaches 0
/// 8. `recover_local_from_hp` - Clears the local player's death once its HP rises above 0
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
        // Register combat presentation messages.
        app.add_message::<super::events::DisplayDamageNumber>();

        // Add the auto-plugin that collects combat systems.
        app.add_plugins(CombatDomainPlugin);

        debug!("CombatPlugin initialized");
    }
}
