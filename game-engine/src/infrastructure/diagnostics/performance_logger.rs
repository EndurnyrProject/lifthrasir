use bevy::diagnostic::{
    Diagnostic, DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Resource)]
#[auto_init_resource(plugin = crate::infrastructure::diagnostics::RoDiagnosticsPlugin)]
pub struct PerformanceLogTimer {
    timer: Timer,
}

impl Default for PerformanceLogTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(5.0, TimerMode::Repeating),
        }
    }
}

fn log_diagnostic_value(diagnostic: &Diagnostic, label: &str, multiplier: f64, unit: &str) {
    if let Some(smoothed) = diagnostic.smoothed() {
        debug!(
            "{}: {:.2}{} (avg: {:.2}{})",
            label,
            diagnostic.value().unwrap_or(0.0) * multiplier,
            unit,
            smoothed * multiplier,
            unit
        );
    }
}

fn log_diagnostic_smoothed(diagnostic: &Diagnostic, label: &str, format: &str) {
    if let Some(smoothed) = diagnostic.smoothed() {
        debug!(
            "{}: {}",
            label,
            format.replace("{}", &format!("{:.0}", smoothed))
        );
    }
}

#[auto_add_system(
    plugin = crate::infrastructure::diagnostics::RoDiagnosticsPlugin,
    schedule = Update
)]
pub fn log_performance_metrics(
    time: Res<Time>,
    mut timer: ResMut<PerformanceLogTimer>,
    diagnostics: Res<DiagnosticsStore>,
) {
    if !timer.timer.tick(time.delta()).just_finished() {
        return;
    }

    debug!("=== Performance Metrics ===");

    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        log_diagnostic_value(fps, "FPS", 1.0, "");
    }

    if let Some(frame_time) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        log_diagnostic_value(frame_time, "Frame Time", 1000.0, "ms");
    }

    if let Some(entity_count) = diagnostics.get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT) {
        log_diagnostic_smoothed(entity_count, "Entity Count", "{}");
    }

    let mut gpu_diagnostics_found = false;
    for diagnostic in diagnostics.iter() {
        let path_str = diagnostic.path().as_str();
        if path_str.contains("gpu_time") {
            if let Some(smoothed) = diagnostic.smoothed() {
                debug!("GPU Time ({}): {:.2}ms", path_str, smoothed * 1000.0);
                gpu_diagnostics_found = true;
            }
        }
    }

    if !gpu_diagnostics_found {
        debug!("GPU Time: Not available (platform may not support GPU timing)");
    }

    debug!("========================");
}
