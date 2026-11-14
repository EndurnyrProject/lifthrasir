//! Death dialog: the modal shown while the local player is dead, after the
//! `Death Dialog` artboard.
//!
//! Spawned when the [`LocalPlayer`] gains [`DeadEntity`], despawned when it loses it.
//! The combat domain's HP-based recovery removes `DeadEntity` on respawn (the aesir
//! server restores HP instead of sending a respawn event), so the
//! `RemovedComponents<DeadEntity>` teardown covers both respawn paths.
//!
//! The two rows write [`RespawnRequested`]: "Return to save point" (`type_ 0`) and
//! "Character Select" (`type_ 1`). They only send the request: the dialog is torn down
//! by the `DeadEntity` removal that recovery drives, not by the click.
//!
//! NOTE: the artboard also shows the killer, the EXP lost, a party member's revive
//! offer and the Token of Siegfried row. The server sends none of the first three and
//! has no request to revive with the token, so they are left out until it does.

use bevy::prelude::*;
use bevy::time::Stopwatch;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::combat::components::DeadEntity;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    WINDOW_SHADOW, display_text, window_title_bar, window_titled,
};
use net_contract::commands::RespawnRequested;

use crate::screens::shared::{RowTone, menu_row, modal_layer, overline, vignette};

/// Renders over the in-game HUD, but one tier *below* the system dialog
/// (`i32::MAX - 2`): on the char-select path the server disconnects and the
/// disconnect system dialog opens over this one, so it must stack above and stay
/// clickable, otherwise the player soft-locks with an unreachable "OK".
const DIALOG_Z: i32 = i32::MAX - 3;

const _: () = assert!(
    DIALOG_Z < crate::widgets::system_dialog::DIALOG_Z,
    "death dialog must stack under the system dialog so the disconnect modal stays clickable",
);
const DIALOG_WIDTH: f32 = 470.0;
const ROW_HEIGHT: f32 = 36.0;

pub struct DeathDialogPlugin;

impl Plugin for DeathDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (show_death_dialog, hide_death_dialog, count_time_fallen),
        );
    }
}

/// The modal's layer, timing how long the hero has been down.
#[derive(Component, Default, Clone)]
pub struct DeathDialogRoot {
    fallen: Stopwatch,
}

/// The text in the title bar that says how long ago the hero fell.
#[derive(Component, Default, Clone)]
struct FallenClock;

/// What a row of the dialog asks the server for.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DeathAction {
    #[default]
    SavePoint,
    CharacterSelect,
}

impl DeathAction {
    /// The `type_` of the [`RespawnRequested`] it sends.
    fn respawn_type(self) -> u32 {
        match self {
            Self::SavePoint => 0,
            Self::CharacterSelect => 1,
        }
    }
}

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

/// How long ago the hero fell: seconds for the first minute, then minutes.
fn fallen_label(secs: u64) -> String {
    match secs {
        60.. => format!("{} min ago", secs / 60),
        _ => format!("{secs} s ago"),
    }
}

/// Keeps the title bar's "N s ago" current.
fn count_time_fallen(
    time: Res<Time>,
    mut roots: Query<&mut DeathDialogRoot>,
    mut clocks: Query<&mut Text, With<FallenClock>>,
) {
    let Ok(mut root) = roots.single_mut() else {
        return;
    };
    let label = fallen_label(root.fallen.tick(time.delta()).elapsed().as_secs());
    for mut text in &mut clocks {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

fn on_death_button(
    activate: On<Activate>,
    buttons: Query<&DeathAction>,
    mut respawn: MessageWriter<RespawnRequested>,
) {
    if let Ok(action) = buttons.get(activate.entity) {
        respawn.write(RespawnRequested {
            type_: action.respawn_type(),
        });
    }
}

/// The whole modal: a blood-red shade closing in on the screen, the headline and the
/// window.
fn death_dialog() -> impl Scene {
    let blood = Color::srgb_u8(0x3c, 0x0a, 0x08);
    let shade = vignette(blood.with_alpha(0.0), blood.with_alpha(0.75), 30.0);
    bsn! {
        @modal_layer(DIALOG_Z)
        DeathDialogRoot
        shade
        Children [
            @headline()
            --
            @window()
        ]
    }
}

fn headline() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(70),
            left: px(0),
            width: percent(100),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            @display_text("You Have Fallen".into(), 40.0, ro_dark::FALLEN)
            TextShadow { offset: {Vec2::new(0.0, 2.0)}, color: Color::BLACK }
        ]
    }
}

fn window() -> impl Scene {
    let since = bsn! {
        FallenClock
        @chrome_text(fallen_label(0), 11.0, ro_dark::MUTED)
    };
    bsn! {
        @window_titled()
        Node {
            width: px(DIALOG_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Children [
            @window_title_bar("You have died", since)
            --
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                margin: {UiRect::top(px(8))},
            }
            @ignore_picking()
            Children [
                @overline("COMING BACK".into(), ro_dark::ACCENT)
                Node { margin: {UiRect::bottom(px(4))} }
                --
                @menu_row(RowTone::Primary, ROW_HEIGHT, "pin", "Return to save point", None)
                DeathAction::SavePoint
                on(on_death_button)
                --
                @menu_row(RowTone::Ghost, ROW_HEIGHT, "members", "Character Select", None)
                DeathAction::CharacterSelect
                on(on_death_button)
            ]
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use std::time::Duration;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<RespawnRequested>();
        app.add_systems(
            Update,
            (show_death_dialog, hide_death_dialog, count_time_fallen),
        );
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
        let world = app.world_mut();
        let actions: Vec<_> = world.query::<&DeathAction>().iter(world).copied().collect();
        assert_eq!(
            actions,
            [DeathAction::SavePoint, DeathAction::CharacterSelect]
        );
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
    fn the_title_bar_counts_the_time_since_the_fall() {
        let mut app = test_app();
        app.world_mut().spawn((LocalPlayer, DeadEntity));
        app.update();
        let world = app.world_mut();
        world
            .query::<&mut DeathDialogRoot>()
            .single_mut(world)
            .unwrap()
            .fallen
            .tick(Duration::from_secs(42));
        app.update();

        let world = app.world_mut();
        let clock = world
            .query_filtered::<&Text, With<FallenClock>>()
            .single(world)
            .unwrap();
        assert_eq!(clock.0, "42 s ago");
    }

    #[test]
    fn the_time_fallen_turns_to_minutes_after_one() {
        assert_eq!(fallen_label(0), "0 s ago");
        assert_eq!(fallen_label(59), "59 s ago");
        assert_eq!(fallen_label(60), "1 min ago");
        assert_eq!(fallen_label(185), "3 min ago");
    }

    #[test]
    fn each_row_requests_its_respawn_type() {
        for (action, expected) in [
            (DeathAction::SavePoint, 0),
            (DeathAction::CharacterSelect, 1),
        ] {
            let mut app = App::new();
            app.add_message::<RespawnRequested>();
            let button = app.world_mut().spawn(action).observe(on_death_button).id();
            app.world_mut().trigger(Activate { entity: button });

            let messages = app.world().resource::<Messages<RespawnRequested>>();
            let sent: Vec<_> = messages.iter_current_update_messages().collect();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].type_, expected);
        }
    }
}
