//! Shared input-capture policy for editor tools and the free camera.
use bevy::{camera_controller::free_camera::FreeCameraState, prelude::*};
use dear_imgui_bevy::prelude::*;

/// Optional so independent, device-free editor systems/tests need no UI host.
pub(in crate::presentation) fn viewport_input_allowed(
    capture: Option<Res<ImguiInputCapture>>,
) -> bool {
    capture.is_none_or(|capture| capture_allows_viewport(capture.primary()))
}

fn capture_allows_viewport(capture: ImguiInputCaptureState) -> bool {
    !capture.wants_any_input()
}

pub(in crate::presentation) fn pointer_captured(capture: Option<&ImguiInputCapture>) -> bool {
    capture.is_some_and(|capture| capture.primary().wants_pointer_input())
}

pub(super) fn sync_camera_input(
    _frame: ImguiFrame<'_>,
    capture: Res<ImguiInputCapture>,
    mut cameras: Query<&mut FreeCameraState>,
) {
    let input_allowed = capture_allows_viewport(capture.primary());
    for mut camera in &mut cameras {
        camera.enabled = input_allowed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_capture_blocks_viewport_input() {
        assert!(viewport_input_allowed(None));
        assert!(capture_allows_viewport(ImguiInputCaptureState::default()));
        for capture in [
            ImguiInputCaptureState {
                want_capture_mouse: true,
                ..default()
            },
            ImguiInputCaptureState {
                want_capture_keyboard: true,
                ..default()
            },
            ImguiInputCaptureState {
                want_text_input: true,
                ..default()
            },
        ] {
            assert!(!capture_allows_viewport(capture));
        }
    }
}
