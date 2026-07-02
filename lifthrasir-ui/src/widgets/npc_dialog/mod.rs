//! NPC dialogue window: an event-driven `bsn!` window that renders each frame the
//! server sends during an NPC conversation (design `2026-07-02-npc-interaction`).
//!
//! The window spawns on the first `NpcDialogReceived` and rebuilds only its body
//! region on later frames (the chrome — wrapper, card, titlebar — persists for the
//! whole conversation). All five `expect` frames are wired: `NEXT`, `CLOSE`, `MENU`,
//! and the `INPUT_INT`/`INPUT_STR` text-field bodies.

use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use bevy_feathers::FeathersCorePlugin;
use bevy_feathers::FeathersPlugins;
use game_engine::core::state::GameState;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::registry::EntityRegistry;
use net_contract::commands::RespondToNpc;
use net_contract::dto::{NpcDialogExpect, NpcResponse};
use net_contract::events::NpcDialogReceived;

use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;

/// Window-root marker: the outer, full-width centering wrapper. A single instance
/// exists for the lifetime of a conversation.
#[derive(Component, Default, Clone)]
pub struct NpcDialogRoot;

/// Captures the card entity (the visible window under the wrapper), so a later
/// frame can reparent a fresh body under it without re-spawning the chrome.
#[derive(Component, FromTemplate, Clone)]
pub struct NpcDialogParts {
    pub card: Entity,
}

/// The titlebar's title text.
#[derive(Component, Default, Clone)]
pub struct NpcDialogTitle;

/// The swappable text + footer region, despawned and rebuilt on every frame.
#[derive(Component, Default, Clone)]
pub struct NpcDialogBody;

/// Marks the `EditableText` field of an `INPUT_INT`/`INPUT_STR` body, so `Confirm`
/// can read its current value.
#[derive(Component, Default, Clone)]
pub struct NpcInputField;

/// What a footer/titlebar button does when activated.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterButtonAction {
    #[default]
    Continue,
    CloseOrCancel,
    /// A MENU option; carries the already-1-based choice index.
    Choice(u32),
    /// Submits the `INPUT_INT`/`INPUT_STR` field's current value.
    Confirm,
}

/// Present only while a conversation is live: the source of truth for the `npc_id`
/// echoed in `RespondToNpc` and the frame currently on screen.
#[derive(Resource, Clone)]
pub struct ActiveNpcDialog {
    pub npc_id: u32,
    pub expect: NpcDialogExpect,
}

const FALLBACK_TITLE: &str = "Conversation";

pub struct NpcDialogPlugin;

impl Plugin for NpcDialogPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(
            Update,
            on_dialog_received.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            cancel_on_escape
                .run_if(in_state(GameState::InGame).and_then(resource_exists::<ActiveNpcDialog>)),
        );
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ActiveNpcDialog>()
        });
    }
}

/// Consumes the latest [`NpcDialogReceived`]: spawns the window on the first frame
/// of a conversation, or rebuilds the body region for a live one. Always refreshes
/// the title and (re)inserts [`ActiveNpcDialog`] for the frame's `expect`.
fn on_dialog_received(
    mut events: MessageReader<NpcDialogReceived>,
    mut commands: Commands,
    roots: Query<&NpcDialogParts, With<NpcDialogRoot>>,
    bodies: Query<Entity, With<NpcDialogBody>>,
    mut titles: Query<&mut Text, With<NpcDialogTitle>>,
    registry: Res<EntityRegistry>,
    names: Query<&EntityName>,
) {
    let Some(event) = events.read().last() else {
        return;
    };

    let name = registry
        .get_entity(event.npc_id)
        .and_then(|entity| names.get(entity).ok())
        .map(|entity_name| entity_name.name.clone());
    let title = title_or_fallback(name);

    match roots.single() {
        Ok(parts) => {
            for body in &bodies {
                commands.entity(body).despawn();
            }
            commands
                .spawn_scene(scene::body(
                    event.text.clone(),
                    event.expect,
                    event.options.clone(),
                ))
                .insert(ChildOf(parts.card));
            if let Ok(mut text) = titles.single_mut() {
                text.0 = title;
            }
        }
        Err(_) => {
            commands
                .spawn_scene(scene::window(
                    title,
                    event.text.clone(),
                    event.expect,
                    event.options.clone(),
                ))
                .insert(DespawnOnExit(GameState::InGame));
        }
    }

    commands.insert_resource(ActiveNpcDialog {
        npc_id: event.npc_id,
        expect: event.expect,
    });
}

/// The resolved NPC display name, falling back to `"Conversation"` when the entity
/// hasn't been named yet (e.g. a click without a prior hover).
fn title_or_fallback(name: Option<String>) -> String {
    name.unwrap_or_else(|| FALLBACK_TITLE.to_string())
}

/// Shared handler for every footer/titlebar button: `Continue`/`Choice` respond and
/// leave the window open (the server drives the next frame); `Confirm` reads the
/// input field and, if it parses for the active frame, responds and stays open;
/// `CloseOrCancel` despawns locally, sending `Cancel` unless the active frame is
/// already terminal (`CLOSE`).
fn on_footer_button(
    activate: On<Activate>,
    actions: Query<&FooterButtonAction>,
    active: Res<ActiveNpcDialog>,
    roots: Query<Entity, With<NpcDialogRoot>>,
    fields: Query<&EditableText, With<NpcInputField>>,
    mut commands: Commands,
    mut respond: MessageWriter<RespondToNpc>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };

    match *action {
        FooterButtonAction::Continue => {
            respond.write(RespondToNpc {
                npc_id: active.npc_id,
                response: NpcResponse::Continue,
            });
        }
        FooterButtonAction::Choice(n) => {
            respond.write(RespondToNpc {
                npc_id: active.npc_id,
                response: NpcResponse::Choice(n),
            });
        }
        FooterButtonAction::Confirm => {
            let Ok(field) = fields.single() else {
                return;
            };
            let text = field.value().to_string();
            if let Some(response) = confirm_response(active.expect, &text) {
                respond.write(RespondToNpc {
                    npc_id: active.npc_id,
                    response,
                });
            }
        }
        FooterButtonAction::CloseOrCancel => {
            close_or_cancel(&active, &roots, &mut commands, &mut respond);
        }
    }
}

/// Maps a `Confirm` submission to the response to send for the active frame's
/// `expect`: `INPUT_STR` always sends the raw text; `INPUT_INT` parses it as `i64`
/// and sends nothing (`None`) on empty/non-numeric input rather than a malformed
/// command.
fn confirm_response(expect: NpcDialogExpect, text: &str) -> Option<NpcResponse> {
    match expect {
        NpcDialogExpect::InputStr => Some(NpcResponse::Input(text.to_string())),
        NpcDialogExpect::InputInt => text.trim().parse::<i64>().ok().map(NpcResponse::Number),
        NpcDialogExpect::Next | NpcDialogExpect::Menu | NpcDialogExpect::Close => None,
    }
}

/// ESC ends the conversation exactly like the titlebar close-dot / a `Close`
/// footer button: `Cancel` unless the active frame is already terminal (`CLOSE`),
/// then despawn. Gated on `ActiveNpcDialog` existing, so it only fires while a
/// dialogue is open; `settings_window`'s own Escape toggle is separately gated to
/// skip while this resource is present, so the two never both react to one press.
fn cancel_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveNpcDialog>,
    roots: Query<Entity, With<NpcDialogRoot>>,
    mut commands: Commands,
    mut respond: MessageWriter<RespondToNpc>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    close_or_cancel(&active, &roots, &mut commands, &mut respond);
}

/// Sends `Cancel` unless the frame is already terminal (`CLOSE`), despawns the
/// window, and clears `ActiveNpcDialog`. Shared by the footer/titlebar close
/// button and the Escape key.
fn close_or_cancel(
    active: &ActiveNpcDialog,
    roots: &Query<Entity, With<NpcDialogRoot>>,
    commands: &mut Commands,
    respond: &mut MessageWriter<RespondToNpc>,
) {
    if active.expect != NpcDialogExpect::Close {
        respond.write(RespondToNpc {
            npc_id: active.npc_id,
            response: NpcResponse::Cancel,
        });
    }
    if let Ok(root) = roots.single() {
        commands.entity(root).despawn();
    }
    commands.remove_resource::<ActiveNpcDialog>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_or_fallback_uses_resolved_name() {
        assert_eq!(
            title_or_fallback(Some("Turban Thief".to_string())),
            "Turban Thief"
        );
    }

    #[test]
    fn title_or_fallback_defaults_when_unresolved() {
        assert_eq!(title_or_fallback(None), FALLBACK_TITLE);
    }

    #[test]
    fn confirm_response_input_str_sends_raw_text() {
        assert_eq!(
            confirm_response(NpcDialogExpect::InputStr, "hello"),
            Some(NpcResponse::Input("hello".to_string()))
        );
    }

    #[test]
    fn confirm_response_input_str_allows_empty_text() {
        assert_eq!(
            confirm_response(NpcDialogExpect::InputStr, ""),
            Some(NpcResponse::Input(String::new()))
        );
    }

    #[test]
    fn confirm_response_input_int_parses_digits() {
        assert_eq!(
            confirm_response(NpcDialogExpect::InputInt, "42"),
            Some(NpcResponse::Number(42))
        );
    }

    #[test]
    fn confirm_response_input_int_rejects_non_numeric() {
        assert_eq!(confirm_response(NpcDialogExpect::InputInt, "abc"), None);
    }

    #[test]
    fn confirm_response_input_int_rejects_empty() {
        assert_eq!(confirm_response(NpcDialogExpect::InputInt, ""), None);
    }
}
