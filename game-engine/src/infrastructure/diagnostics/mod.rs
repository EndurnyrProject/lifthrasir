mod animation_diagnostics;
mod performance_logger;

pub use animation_diagnostics::*;
pub use performance_logger::*;

use bevy_auto_plugin::prelude::*;

/// Plugin for registering all diagnostic systems
/// Note: FrameTimeDiagnosticsPlugin, EntityCountDiagnosticsPlugin, and RenderDiagnosticsPlugin
/// are already added by Tauri or DefaultPlugins
/// SystemInformationDiagnosticsPlugin is NOT added as it's not supported on macOS/Tauri
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct RoDiagnosticsPlugin;
