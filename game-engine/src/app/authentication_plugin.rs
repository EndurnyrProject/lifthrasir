use bevy_auto_plugin::modes::global::prelude::{auto_plugin, AutoPlugin};

/// Authentication Plugin
///
/// Handles all authentication-related functionality including:
/// - Client configuration loading
/// - Login flow (connect → authenticate → session management)
/// - Server selection
/// - Event-driven state transitions
///
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AuthenticationPlugin;
