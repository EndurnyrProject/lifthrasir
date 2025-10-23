use bevy::input::ButtonInput;
use bevy::prelude::*;
use game_engine::domain::camera::CameraRotationDelta;
use game_engine::domain::input::{ForwardedCursorPosition, ForwardedMouseClick};

use crate::bridge::events::{
    CameraRotationEvent, KeyboardInputEvent, MouseClickEvent, MousePositionEvent,
};

/// Convert JavaScript KeyboardEvent.code to Bevy KeyCode
fn js_code_to_bevy_keycode(code: &str) -> Option<KeyCode> {
    match code {
        // Letters
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        // Arrow keys
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        // Numbers
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "Digit0" => Some(KeyCode::Digit0),
        // Special keys
        "Space" => Some(KeyCode::Space),
        "Enter" => Some(KeyCode::Enter),
        "Escape" => Some(KeyCode::Escape),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "ShiftLeft" | "ShiftRight" => Some(KeyCode::ShiftLeft),
        "ControlLeft" | "ControlRight" => Some(KeyCode::ControlLeft),
        "AltLeft" | "AltRight" => Some(KeyCode::AltLeft),
        _ => None,
    }
}

/// System that handles KeyboardInputEvent
/// Directly updates Bevy's ButtonInput<KeyCode> resource
pub fn handle_keyboard_input(
    mut events: MessageReader<KeyboardInputEvent>,
    mut keyboard_input: ResMut<ButtonInput<KeyCode>>,
) {
    for event in events.read() {
        if let Some(keycode) = js_code_to_bevy_keycode(&event.code) {
            if event.pressed {
                keyboard_input.press(keycode);
            } else {
                keyboard_input.release(keycode);
            }
        }
    }
}

/// System that handles MousePositionEvent
/// Directly updates ForwardedCursorPosition resource
pub fn handle_mouse_position(
    mut events: MessageReader<MousePositionEvent>,
    mut cursor_position: ResMut<ForwardedCursorPosition>,
) {
    for event in events.read() {
        cursor_position.position = Some(Vec2::new(event.x, event.y));
    }
}

/// System that handles MouseClickEvent
/// Updates ForwardedMouseClick resource
pub fn handle_mouse_click(
    mut events: MessageReader<MouseClickEvent>,
    mut mouse_click: ResMut<ForwardedMouseClick>,
) {
    for event in events.read() {
        mouse_click.position = Some(Vec2::new(event.x, event.y));
    }
}

/// System that handles CameraRotationEvent
/// Updates CameraRotationDelta resource
pub fn handle_camera_rotation(
    mut events: MessageReader<CameraRotationEvent>,
    mut rotation_delta: ResMut<CameraRotationDelta>,
) {
    for event in events.read() {
        rotation_delta.delta_x += event.delta_x;
        rotation_delta.delta_y += event.delta_y;
    }
}
