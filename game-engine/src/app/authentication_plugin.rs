use bevy_auto_plugin::prelude::{auto_add_plugin, AutoPlugin};

#[auto_add_plugin(plugin = AuthenticationPlugin, init)]
use bevy_quinnet::client::QuinnetClientPlugin;

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
