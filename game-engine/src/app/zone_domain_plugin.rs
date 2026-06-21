use bevy_auto_plugin::prelude::*;

/// Plugin for zone domain logic (QUIC zone flow, state machine, and fresh zone events).
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct ZoneDomainAutoPlugin;
