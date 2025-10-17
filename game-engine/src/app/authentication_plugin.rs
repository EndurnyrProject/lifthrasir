use bevy_auto_plugin::modes::global::prelude::{AutoPlugin, auto_plugin};

/// Authentication Plugin
///
/// Handles all authentication-related functionality including:
/// - Client configuration loading
/// - Login flow (connect → authenticate → session management)
/// - Server selection
/// - Event-driven state transitions
///
/// All systems, events, and resources are auto-registered via bevy_auto_plugin attributes.
/// See `domain/authentication/systems.rs` for system implementations.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AuthenticationPlugin;
