use lifthrasir_runtime::actors::{ActionType, ActorAnimationClock, ActorAnimationInput};
use std::time::Duration;

fn request(action: ActionType) -> ActorAnimationInput {
    ActorAnimationInput {
        action,
        restart_id: 1,
        started_at: Duration::ZERO,
        duration: None,
        speed_factor: 1.0,
        paused: false,
    }
}

#[test]
fn identical_timestamps_do_not_hide_new_playback_requests() {
    let mut clock = ActorAnimationClock::default();
    let mut input = request(ActionType::Attack);
    assert!(clock.update(&input, Duration::from_millis(100)));
    assert!(!clock.update(&input, Duration::from_millis(200)));
    input.restart_id += 1;
    assert!(clock.update(&input, Duration::from_millis(200)));
    assert_eq!(clock.elapsed(), Duration::from_millis(200));
}

#[test]
fn pause_and_delay_factor_changes_preserve_progress_without_restarting() {
    let mut clock = ActorAnimationClock::default();
    let mut input = request(ActionType::Walk);
    clock.update(&input, Duration::from_millis(1000));
    input.speed_factor = 2.0;
    assert!(!clock.update(&input, Duration::from_millis(1000)));
    assert_eq!(clock.natural_elapsed_ms() / 2.0, 1000.0);
    input.paused = true;
    assert!(!clock.update(&input, Duration::from_millis(1200)));
    assert!(!clock.update(&input, Duration::from_millis(2200)));
    assert_eq!(clock.natural_elapsed_ms(), 2200.0);
    assert_eq!(clock.elapsed(), Duration::from_millis(1200));
    input.paused = false;
    assert!(!clock.update(&input, Duration::from_millis(2200)));
    assert!(!clock.update(&input, Duration::from_millis(2400)));
    assert_eq!(clock.natural_elapsed_ms(), 2400.0);
    assert_eq!(clock.elapsed(), Duration::from_millis(1400));
    assert_eq!(clock.restart_id(), Some(1));
}

#[test]
fn a_paused_pose_defers_new_action_requests_until_resumed() {
    let mut clock = ActorAnimationClock::default();
    let mut input = request(ActionType::Idle);
    clock.update(&input, Duration::from_millis(100));
    input.action = ActionType::Hit;
    input.restart_id = 2;
    input.started_at = Duration::from_millis(100);
    input.paused = true;
    assert!(!clock.update(&input, Duration::from_millis(100)));
    assert_eq!(clock.restart_id(), Some(1));
    input.paused = false;
    assert!(clock.update(&input, Duration::from_millis(200)));
    assert_eq!(clock.restart_id(), Some(2));
}
