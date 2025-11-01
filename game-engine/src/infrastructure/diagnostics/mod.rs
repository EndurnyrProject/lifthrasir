mod animation_diagnostics;
mod performance_logger;

pub use animation_diagnostics::*;
pub use performance_logger::*;

use bevy::prelude::*;

/// Plugin for registering all diagnostic systems
/// Note: FrameTimeDiagnosticsPlugin, EntityCountDiagnosticsPlugin, and RenderDiagnosticsPlugin
/// are already added by Tauri or DefaultPlugins
/// SystemInformationDiagnosticsPlugin is NOT added as it's not supported on macOS/Tauri
pub struct RoDiagnosticsPlugin;

impl Plugin for RoDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationDiagnostics>()
            .init_resource::<PerformanceLogTimer>()
            .add_systems(
                Update,
                (
                    update_animation_diagnostics,
                    log_animation_diagnostics,
                    log_performance_metrics,
                ),
            );
    }
}
