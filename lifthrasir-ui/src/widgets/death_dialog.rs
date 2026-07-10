//! Death dialog: a centered BSN/Feathers modal shown while the local player is dead.
//!
//! Spawned when the [`LocalPlayer`] gains [`DeadEntity`], despawned when it loses it.
//! The combat domain's HP-based recovery removes `DeadEntity` on respawn (there is no
//! `SelfRespawned` event to key off — the aesir server restores HP instead), so the
//! `RemovedComponents<DeadEntity>` teardown covers both respawn paths.
//!
//! Two Feathers buttons write [`RespawnRequested`]: "Return to save point" (`type_ 0`)
//! and "Character Select" (`type_ 1`); the `net-aesir` adapter translates that into the
//! outbound `Respawn` command. The buttons only send the request — the dialog is torn
//! down by the `DeadEntity` removal recovery drives, not by the click.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemedText};
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::combat::components::DeadEntity;
use game_engine::domain::entities::markers::LocalPlayer;
use net_contract::commands::RespawnRequested;

use crate::theme;
use crate::theme::feathers_theme::{install_norse_theme, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER};

/// Renders over the in-game HUD, but one tier *below* the system dialog
/// (`i32::MAX - 2`): on the char-select path the server disconnects and the
/// disconnect system dialog opens over this one, so it must stack above and stay
/// clickable, otherwise the player soft-locks with an unreachable "OK".
const DIALOG_Z: i32 = i32::MAX - 3;

const _: () = assert!(
    DIALOG_Z < crate::widgets::system_dialog::DIALOG_Z,
    "death dialog must stack under the system dialog so the disconnect modal stays clickable",
);
const DIALOG_WIDTH: f32 = 320.0;

pub struct DeathDialogPlugin;

impl Plugin for DeathDialogPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(Update, (show_death_dialog, hide_death_dialog));
    }
}

/// Marker on the modal backdrop root, so show/hide can find and despawn it.
#[derive(Component, Default, Clone)]
pub struct DeathDialogRoot;

/// Spawn the modal the frame the local player enters the dead state, unless one is
/// already open (idempotent against a re-death before teardown).
fn show_death_dialog(
    dead_local: Query<(), (With<LocalPlayer>, Added<DeadEntity>)>,
    existing: Query<(), With<DeathDialogRoot>>,
    mut commands: Commands,
) {
    if dead_local.is_empty() || !existing.is_empty() {
        return;
    }
    commands
        .spawn_scene(death_dialog())
        .insert(DespawnOnExit(GameState::InGame));
}

/// Despawn the modal when the local player's `DeadEntity` is removed. `RemovedComponents`
/// yields the entity the component left; we only tear down for the local player's removal,
/// not some other unit's corpse expiring. If the local player itself was despawned (the
/// char-select disconnect), no `LocalPlayer` remains and `DespawnOnExit` handles cleanup.
fn hide_death_dialog(
    mut removed: RemovedComponents<DeadEntity>,
    local: Query<Entity, With<LocalPlayer>>,
    dialog: Query<Entity, With<DeathDialogRoot>>,
    mut commands: Commands,
) {
    let Ok(local) = local.single() else {
        removed.clear();
        return;
    };
    if removed.read().any(|entity| entity == local) {
        for root in &dialog {
            commands.entity(root).despawn();
        }
    }
}

fn on_return_to_save(_: On<Activate>, mut respawn: MessageWriter<RespawnRequested>) {
    respawn.write(RespawnRequested { type_: 0 });
}

fn on_character_select(_: On<Activate>, mut respawn: MessageWriter<RespawnRequested>) {
    respawn.write(RespawnRequested { type_: 1 });
}

/// The whole modal as one scene: a dimmed, click-eating backdrop centering a glass card.
fn death_dialog() -> impl Scene {
    bsn! {
        DeathDialogRoot
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BackgroundColor({Color::srgba(0.012, 0.027, 0.024, 0.55)})
        GlobalZIndex({DIALOG_Z})
        Pickable
        Children [ card() ]
    }
}

fn card() -> impl Scene {
    bsn! {
        Node {
            width: px(DIALOG_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: px(14),
            padding: {UiRect::axes(px(28), px(26))},
            border: px(1),
            border_radius: BorderRadius::all(px(14)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Children [
            title("You have died"),
            (
                @FeathersButton { @caption: bsn! { button_label("Return to save point") } }
                Node { height: px(40) }
                on(on_return_to_save)
            ),
            (
                @FeathersButton { @caption: bsn! { button_label("Character Select") } }
                Node { height: px(40) }
                on(on_character_select)
            ),
        ]
    }
}

/// Standalone modal heading: Feathers has no font token and this text sits outside a
/// Feathers ancestor, so the font and color are set explicitly.
fn title(text: &'static str) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
            font_size: {FontSize::Px(19.0)},
        }
        TextColor({theme::DISPLAY_GOLD})
        Node { align_self: {AlignSelf::Center}, margin: {UiRect::bottom(px(4))} }
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

/// Button caption: `ThemedText` inherits font + color from the Feathers button ancestor.
fn button_label(text: &'static str) -> impl Scene {
    bsn! {
        Text(text)
        ThemedText
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<RespawnRequested>();
        app.add_systems(Update, (show_death_dialog, hide_death_dialog));
        app
    }

    fn dialog_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<DeathDialogRoot>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn dialog_appears_when_local_player_dies() {
        let mut app = test_app();
        app.world_mut().spawn((LocalPlayer, DeadEntity));
        app.update();

        assert_eq!(dialog_count(&mut app), 1);
    }

    #[test]
    fn no_dialog_without_a_dead_local_player() {
        let mut app = test_app();
        app.world_mut().spawn(LocalPlayer);
        app.world_mut().spawn(DeadEntity);
        app.update();

        assert_eq!(dialog_count(&mut app), 0);
    }

    #[test]
    fn removing_dead_entity_despawns_the_dialog() {
        let mut app = test_app();
        let player = app.world_mut().spawn((LocalPlayer, DeadEntity)).id();
        app.update();
        assert_eq!(dialog_count(&mut app), 1);

        app.world_mut().entity_mut(player).remove::<DeadEntity>();
        app.update();

        assert_eq!(dialog_count(&mut app), 0);
    }

    #[test]
    fn only_one_dialog_when_death_persists_across_frames() {
        let mut app = test_app();
        app.world_mut().spawn((LocalPlayer, DeadEntity));
        app.update();
        app.update();

        assert_eq!(dialog_count(&mut app), 1);
    }

    #[test]
    fn return_to_save_button_requests_type_0() {
        let mut app = App::new();
        app.add_message::<RespawnRequested>();
        let button = app
            .world_mut()
            .spawn_empty()
            .observe(on_return_to_save)
            .id();
        app.world_mut().trigger(Activate { entity: button });

        let messages = app.world().resource::<Messages<RespawnRequested>>();
        let sent: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].type_, 0);
    }

    #[test]
    fn character_select_button_requests_type_1() {
        let mut app = App::new();
        app.add_message::<RespawnRequested>();
        let button = app
            .world_mut()
            .spawn_empty()
            .observe(on_character_select)
            .id();
        app.world_mut().trigger(Activate { entity: button });

        let messages = app.world().resource::<Messages<RespawnRequested>>();
        let sent: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].type_, 1);
    }
}
