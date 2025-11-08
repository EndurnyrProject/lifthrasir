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
}

impl AnimationDiagnostics {
    #[inline]
    pub fn record_conversion(&mut self) {
        self.total_conversions += 1;
        self.current_conversions += 1;
    }

    #[inline]
    pub fn record_skip(&mut self) {
        self.frames_skipped += 1;
    }

    #[inline]
    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    #[inline]
    pub fn record_cache_miss(&mut self) {
        self.cache_misses += 1;
    }

    /// Get cache hit ratio (0.0 to 1.0)
    pub fn cache_hit_ratio(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f32 / total as f32
        }
    }

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
    frame_cache: Option<Res<crate::domain::entities::animation::RoFrameCache>>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();

    if *timer >= 5.0 {
        let cache_ratio = diagnostics.cache_hit_ratio();
        let conversions_per_sec = diagnostics.conversions_per_second();

        if let Some(frame_cache) = frame_cache {
            debug!(
                "Animation Stats: {:.0} conversions/sec, Cache: {:.1}% hit rate ({}/{}), {} cached frames (capacity: {}), Skipped: {} frames",
                conversions_per_sec,
                cache_ratio * 100.0,
                diagnostics.cache_hits,
                diagnostics.cache_hits + diagnostics.cache_misses,
                frame_cache.len(),
                frame_cache.capacity(),
                diagnostics.frames_skipped
            );
        } else {
            debug!(
                "Animation Stats: {:.0} conversions/sec, Cache: {:.1}% hit rate ({}/{}), Skipped: {} frames",
                conversions_per_sec,
                cache_ratio * 100.0,
                diagnostics.cache_hits,
                diagnostics.cache_hits + diagnostics.cache_misses,
                diagnostics.frames_skipped
            );
        }

        *timer = 0.0;
    }
}
