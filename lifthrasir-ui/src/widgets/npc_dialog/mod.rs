//! NPC dialogue window: an event-driven `bsn!` window that renders each frame the
//! server sends during an NPC conversation (design `2026-07-02-npc-interaction`).
//!
//! The window spawns on the first `NpcDialogReceived` and rebuilds only its body
//! region on later frames (the chrome — wrapper, card, titlebar — persists for the
//! whole conversation). `NEXT` and `CLOSE` are the only interactive frames this task
//! wires; `MENU`/`INPUT_INT`/`INPUT_STR` render as a text-only placeholder so a
//! conversation never panics ahead of Tasks 7/8.

use bevy::prelude::*;
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

/// What a footer/titlebar button does when activated.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterButtonAction {
    #[default]
    Continue,
    CloseOrCancel,
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
                .spawn_scene(scene::body(event.text.clone(), event.expect))
                .insert(ChildOf(parts.card));
            if let Ok(mut text) = titles.single_mut() {
                text.0 = title;
            }
        }
        Err(_) => {
            commands
                .spawn_scene(scene::window(title, event.text.clone(), event.expect))
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

/// Shared handler for every footer/titlebar button: `Continue` responds and leaves
/// the window open (the server drives the next frame); `CloseOrCancel` despawns
/// locally, sending `Cancel` unless the active frame is already terminal (`CLOSE`).
fn on_footer_button(
    activate: On<Activate>,
    actions: Query<&FooterButtonAction>,
    active: Res<ActiveNpcDialog>,
    roots: Query<Entity, With<NpcDialogRoot>>,
    mut commands: Commands,
    mut respond: MessageWriter<RespondToNpc>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };

    if *action == FooterButtonAction::Continue {
        respond.write(RespondToNpc {
            npc_id: active.npc_id,
            response: NpcResponse::Continue,
        });
        return;
    }

    close_or_cancel(&active, &roots, &mut commands, &mut respond);
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
}
