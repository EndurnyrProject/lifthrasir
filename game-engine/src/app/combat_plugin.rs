use bevy_auto_plugin::prelude::*;

/// Combat Domain Plugin
///
/// This auto-plugin handles combat systems.
/// Network message registration is handled by the CombatPlugin wrapper
/// in domain/combat/plugin.rs.
///
/// Registered systems:
/// - process_combat_actions
/// - handle_hit_reactions
/// - update_attack_timers
/// - update_hit_stun
/// - handle_death
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct CombatDomainPlugin;
