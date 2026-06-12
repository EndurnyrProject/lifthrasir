use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use std::collections::VecDeque;

/// Number of seconds to keep in diagnostics history
const DIAGNOSTICS_HISTORY_SECONDS: usize = 60;

/// Resource for tracking animation performance metrics
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::infrastructure::diagnostics::RoDiagnosticsPlugin)]
pub struct AnimationDiagnostics {
    /// Total number of palette conversions performed
    pub total_conversions: u64,
    /// Number of frames skipped (early exits)
    pub frames_skipped: u64,
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    conversions_history: VecDeque<u64>,
    /// Time accumulator for per-second tracking
    time_accumulator: f32,
    /// Current conversions in this second
    current_conversions: u64,
    /// Last logged head direction (for change detection)
    pub last_head_direction: Option<usize>,
}

impl AnimationDiagnostics {
    pub fn conversions_per_second(&self) -> f32 {
        if self.conversions_history.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.conversions_history.iter().sum();
        sum as f32 / self.conversions_history.len() as f32
    }
}

#[auto_add_system(
    plugin = crate::infrastructure::diagnostics::RoDiagnosticsPlugin,
    schedule = Update
)]
pub fn update_animation_diagnostics(
    time: Res<Time>,
    mut diagnostics: ResMut<AnimationDiagnostics>,
) {
    diagnostics.time_accumulator += time.delta_secs();

    if diagnostics.time_accumulator >= 1.0 {
        let current = diagnostics.current_conversions;
        diagnostics.conversions_history.push_back(current);
        if diagnostics.conversions_history.len() > DIAGNOSTICS_HISTORY_SECONDS {
            diagnostics.conversions_history.pop_front();
        }

        diagnostics.current_conversions = 0;
        diagnostics.time_accumulator = 0.0;
    }
}

#[auto_add_system(
    plugin = crate::infrastructure::diagnostics::RoDiagnosticsPlugin,
    schedule = Update
)]
pub fn log_animation_diagnostics(
    diagnostics: Res<AnimationDiagnostics>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();

    if *timer >= 5.0 {
        let conversions_per_sec = diagnostics.conversions_per_second();

        debug!(
            "Animation Stats: {:.0} conversions/sec, Skipped: {} frames",
            conversions_per_sec, diagnostics.frames_skipped
        );

        *timer = 0.0;
    }
}
