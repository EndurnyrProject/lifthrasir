use bevy_auto_plugin::prelude::AutoPlugin;

/// Root auto-plugin; app-level states register themselves via `auto_*`
/// attributes in `core/state.rs`.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct LifthrasirPlugin;
