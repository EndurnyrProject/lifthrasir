//! Pure, headlessly-testable logic behind the status-icon bar: the diff that keeps
//! icon children in sync with active statuses, the remaining-time formatter, and the
//! near-expiry blink curve. The ECS wiring in `mod.rs` stays thin around these.

use std::collections::HashSet;
use std::time::Duration;

/// Side length in px of a single status icon (and its placeholder tile).
pub const ICON_SIZE: f32 = 32.0;

/// A timed status starts blinking once its remaining time drops below this.
pub const BLINK_THRESHOLD: Duration = Duration::from_secs(8);

/// Alpha never dips below this fraction of the icon's resting alpha while blinking,
/// so a blinking icon stays legible.
const BLINK_MIN_ALPHA: f32 = 0.3;

/// Angular speed of the blink oscillation in rad/s (~one pulse per second).
const BLINK_SPEED: f32 = 6.0;

/// Human-readable remaining time: `"58s"` under a minute, `"4m"` on an exact
/// minute, `"4m30s"` otherwise.
pub fn format_remaining(remaining: Duration) -> String {
    let secs = remaining.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    let (m, s) = (secs / 60, secs % 60);
    if s == 0 {
        format!("{m}m")
    } else {
        format!("{m}m{s}s")
    }
}

/// Given the set of currently-active EFSTs and the set already rendered as icon
/// children, returns `(to_add, to_remove)` sorted for deterministic spawning.
pub fn diff_efsts(active: &HashSet<u32>, existing: &HashSet<u32>) -> (Vec<u32>, Vec<u32>) {
    let mut to_add: Vec<u32> = active.difference(existing).copied().collect();
    let mut to_remove: Vec<u32> = existing.difference(active).copied().collect();
    to_add.sort_unstable();
    to_remove.sort_unstable();
    (to_add, to_remove)
}

/// Blink alpha *factor* in `[BLINK_MIN_ALPHA, 1.0]`, applied on top of an icon's
/// resting alpha. Permanent statuses and timed statuses still above the threshold
/// return `1.0` (no blink); inside the final window it oscillates on `elapsed`.
pub fn blink_alpha(remaining: Option<Duration>, permanent: bool, elapsed_secs: f32) -> f32 {
    if permanent {
        return 1.0;
    }
    let Some(remaining) = remaining else {
        return 1.0;
    };
    if remaining >= BLINK_THRESHOLD {
        return 1.0;
    }
    let wave = (elapsed_secs * BLINK_SPEED).sin() * 0.5 + 0.5;
    BLINK_MIN_ALPHA + (1.0 - BLINK_MIN_ALPHA) * wave
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_remaining_under_a_minute_is_seconds() {
        assert_eq!(format_remaining(Duration::from_secs(0)), "0s");
        assert_eq!(format_remaining(Duration::from_secs(58)), "58s");
        assert_eq!(format_remaining(Duration::from_secs(59)), "59s");
    }

    #[test]
    fn format_remaining_exact_minute_drops_seconds() {
        assert_eq!(format_remaining(Duration::from_secs(60)), "1m");
        assert_eq!(format_remaining(Duration::from_secs(4 * 60)), "4m");
    }

    #[test]
    fn format_remaining_minutes_and_seconds() {
        assert_eq!(format_remaining(Duration::from_secs(4 * 60 + 30)), "4m30s");
        assert_eq!(format_remaining(Duration::from_secs(90)), "1m30s");
    }

    fn set(items: &[u32]) -> HashSet<u32> {
        items.iter().copied().collect()
    }

    #[test]
    fn diff_adds_new_and_keeps_existing() {
        let (add, remove) = diff_efsts(&set(&[1, 2, 3]), &set(&[2]));
        assert_eq!(add, vec![1, 3]);
        assert!(remove.is_empty());
    }

    #[test]
    fn diff_removes_gone() {
        let (add, remove) = diff_efsts(&set(&[2]), &set(&[1, 2]));
        assert!(add.is_empty());
        assert_eq!(remove, vec![1]);
    }

    #[test]
    fn diff_no_change() {
        let (add, remove) = diff_efsts(&set(&[1, 2]), &set(&[1, 2]));
        assert!(add.is_empty());
        assert!(remove.is_empty());
    }

    #[test]
    fn blink_alpha_permanent_is_full() {
        assert_eq!(blink_alpha(None, true, 3.7), 1.0);
        assert_eq!(blink_alpha(Some(Duration::from_secs(2)), true, 3.7), 1.0);
    }

    #[test]
    fn blink_alpha_above_threshold_is_full() {
        assert_eq!(blink_alpha(Some(Duration::from_secs(30)), false, 3.7), 1.0);
        assert_eq!(blink_alpha(Some(BLINK_THRESHOLD), false, 3.7), 1.0);
    }

    #[test]
    fn blink_alpha_none_remaining_is_full() {
        assert_eq!(blink_alpha(None, false, 3.7), 1.0);
    }

    #[test]
    fn blink_alpha_near_expiry_oscillates_below_full() {
        let a = blink_alpha(Some(Duration::from_secs(2)), false, 0.0);
        assert!(a < 1.0, "expected blink below full alpha, got {a}");
        assert!(
            a >= BLINK_MIN_ALPHA,
            "expected blink at or above min, got {a}"
        );
    }
}
