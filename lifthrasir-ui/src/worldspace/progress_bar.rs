//! NPC `progressbar`: a screen-projected fill bar above the local player's head.
//!
//! Spawned on [`ProgressBarStarted`] (the `progressbar` script buildin), the bar
//! fills over `seconds` and is tinted by the script's `color` (0xRRGGBB, rendered
//! verbatim — `0` is a black bar). On natural completion the client acks
//! `NpcResponse::Progress`; a movement request or ESC while it runs acks
//! `NpcResponse::Cancel` instead, closing any open dialogue window too. The
//! server's `npc_id` addresses the ack, so a bare `progressbar` with no dialogue
//! window still completes and cancels correctly.
//!
//! While a bar is live, [`ActiveProgressBar`] is present; the NPC dialogue
//! widget's own ESC handler is gated off against it so ESC fires exactly one
//! `Cancel`.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::markers::LocalPlayer;
use net_contract::commands::{MoveRequested, RespondToNpc};
use net_contract::dto::NpcResponse;
use net_contract::events::ProgressBarStarted;

use crate::theme;
use crate::widgets::npc_dialog::{ActiveNpcDialog, NpcDialogRoot};
use crate::worldspace::{WorldCameraFilter, viewport_to_ui};

const BAR_WIDTH: f32 = 130.0;
const BAR_HEIGHT: f32 = 7.0;
/// Pixels above the player's projected origin; matches the skill cast bar so the
/// two read at the same height over the head.
const BAR_HEAD_GAP: f32 = 88.0;
/// Just above the skill cast bar (160) so a progressbar reads over a stray cast.
const BAR_Z: i32 = 161;

/// Present only while a `progressbar` is live. Carries the owning `npc_id` and
/// gates the NPC dialogue widget's ESC handler so the bar owns cancel.
#[derive(Resource, Clone, Copy)]
pub struct ActiveProgressBar {
    pub npc_id: u32,
}

/// The worldspace bar root, following `target`'s head. `npc_id` addresses the
/// Progress/Cancel ack; `timer` drives both the fill and completion.
#[derive(Component)]
struct NpcProgressBar {
    target: Entity,
    npc_id: u32,
    timer: Timer,
}

/// Fill node of a progress bar; width tracks the bar's timer fraction.
#[derive(Component)]
struct ProgressBarFill;

pub struct NpcProgressBarPlugin;

impl Plugin for NpcProgressBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_progress_bar,
                complete_progress_bar,
                cancel_progress_bar.run_if(resource_exists::<ActiveProgressBar>),
                fill_progress_bar,
                follow_progress_bar,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all_progress_bars);
    }
}

/// `0xRRGGBB` -> `Color`, rendered verbatim (`0` is black; no default fallback).
fn bar_color(rgb: u32) -> Color {
    let r = ((rgb >> 16) & 0xff) as u8;
    let g = ((rgb >> 8) & 0xff) as u8;
    let b = (rgb & 0xff) as u8;
    Color::srgb_u8(r, g, b)
}

/// Spawn the bar on the invoking player's head. A new bar replaces any live one
/// (the player's script suspends on a single bar), and `ActiveProgressBar` is
/// (re)inserted so the dialogue widget yields ESC.
fn spawn_progress_bar(
    mut events: MessageReader<ProgressBarStarted>,
    mut commands: Commands,
    player: Query<Entity, With<LocalPlayer>>,
    existing: Query<Entity, With<NpcProgressBar>>,
) {
    for event in events.read() {
        if event.seconds == 0 {
            continue;
        }
        let Ok(target) = player.single() else {
            continue;
        };
        for bar in &existing {
            commands.entity(bar).despawn();
        }
        spawn_bar(
            &mut commands,
            target,
            event.npc_id,
            event.seconds,
            event.color,
        );
        commands.insert_resource(ActiveProgressBar {
            npc_id: event.npc_id,
        });
    }
}

fn spawn_bar(commands: &mut Commands, target: Entity, npc_id: u32, seconds: u32, color: u32) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(BAR_WIDTH),
            height: Val::Px(BAR_HEIGHT),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(theme::FIELD),
        BorderColor::all(theme::GOLD_FAINT),
        GlobalZIndex(BAR_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
        NpcProgressBar {
            target,
            npc_id,
            timer: Timer::from_seconds(seconds as f32, TimerMode::Once),
        },
        children![(
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(bar_color(color)),
            ProgressBarFill,
            Pickable::IGNORE,
        )],
    ));
}

/// Advance the timer; on completion ack `Progress`, despawn, and drop the gate.
fn complete_progress_bar(
    time: Res<Time>,
    mut bars: Query<(Entity, &mut NpcProgressBar)>,
    mut commands: Commands,
    mut respond: MessageWriter<RespondToNpc>,
) {
    for (entity, mut bar) in &mut bars {
        bar.timer.tick(time.delta());
        if !bar.timer.is_finished() {
            continue;
        }
        respond.write(RespondToNpc {
            npc_id: bar.npc_id,
            response: NpcResponse::Progress,
        });
        commands.entity(entity).despawn();
        commands.remove_resource::<ActiveProgressBar>();
    }
}

/// A movement request or ESC while the bar runs acks `Cancel`, despawns the bar,
/// and closes any open dialogue window. The triggering move is left in its
/// Messages buffer, so the network send system still dispatches it — the player
/// walks. Gated on `ActiveProgressBar`, so it only fires while a bar is live.
fn cancel_progress_bar(
    keys: Res<ButtonInput<KeyCode>>,
    mut moves: MessageReader<MoveRequested>,
    bars: Query<(Entity, &NpcProgressBar)>,
    roots: Query<Entity, With<NpcDialogRoot>>,
    active_dialog: Option<Res<ActiveNpcDialog>>,
    mut commands: Commands,
    mut respond: MessageWriter<RespondToNpc>,
) {
    let moved = moves.read().next().is_some();
    if !moved && !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    for (entity, bar) in &bars {
        respond.write(RespondToNpc {
            npc_id: bar.npc_id,
            response: NpcResponse::Cancel,
        });
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<ActiveProgressBar>();
    // Close the whole interaction: the bar owns cancel, so tear down any dialogue
    // window here rather than leaving it stranded (its own ESC handler is gated
    // off while the bar is live).
    if active_dialog.is_some() {
        for root in &roots {
            commands.entity(root).despawn();
        }
        commands.remove_resource::<ActiveNpcDialog>();
    }
}

/// Grow each bar's fill with its timer, the same timer that completes it.
fn fill_progress_bar(
    bars: Query<&NpcProgressBar>,
    parents: Query<&ChildOf>,
    mut fills: Query<(Entity, &mut Node), With<ProgressBarFill>>,
) {
    for (entity, mut node) in &mut fills {
        let Some(bar) = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| bars.get(ancestor).ok())
        else {
            continue;
        };
        node.width = Val::Percent(bar.timer.fraction() * 100.0);
    }
}

/// Project the bar onto the player's head each frame; despawn if the target is
/// gone (e.g. a map change removed the local player entity).
fn follow_progress_bar(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
    mut bars: Query<(Entity, &NpcProgressBar, &mut Node, &mut Visibility)>,
    mut commands: Commands,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, bar, mut node, mut visibility) in &mut bars {
        let Ok(target_transform) = targets.get(bar.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(camera_transform, target_transform.translation()) {
            Ok(screen) => {
                let pos = viewport_to_ui(screen, &ui_scale);
                node.left = Val::Px(pos.x - BAR_WIDTH / 2.0);
                node.top = Val::Px(pos.y - BAR_HEAD_GAP);
                *visibility = Visibility::Visible;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

fn despawn_all_progress_bars(mut commands: Commands, bars: Query<Entity, With<NpcProgressBar>>) {
    for entity in &bars {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<ActiveProgressBar>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_color_decodes_rrggbb() {
        assert_eq!(bar_color(0xffff00), Color::srgb_u8(0xff, 0xff, 0x00));
        assert_eq!(bar_color(0x000000), Color::srgb_u8(0, 0, 0));
        assert_eq!(bar_color(0x3366cc), Color::srgb_u8(0x33, 0x66, 0xcc));
    }

    #[test]
    fn zero_duration_bar_is_ignored() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<ProgressBarStarted>();
        let player = app.world_mut().spawn(LocalPlayer).id();
        let _ = player;
        app.add_systems(Update, spawn_progress_bar);

        app.world_mut()
            .resource_mut::<Messages<ProgressBarStarted>>()
            .write(ProgressBarStarted {
                seconds: 0,
                color: 0xffff00,
                npc_id: 7,
            });
        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&NpcProgressBar>().iter(world).count(), 0);
        assert!(app.world().get_resource::<ActiveProgressBar>().is_none());
    }

    #[test]
    fn started_bar_spawns_on_local_player_and_sets_gate() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<ProgressBarStarted>();
        let player = app.world_mut().spawn(LocalPlayer).id();
        app.add_systems(Update, spawn_progress_bar);

        app.world_mut()
            .resource_mut::<Messages<ProgressBarStarted>>()
            .write(ProgressBarStarted {
                seconds: 3,
                color: 0xffff00,
                npc_id: 150001,
            });
        app.update();

        let world = app.world_mut();
        let mut q = world.query::<&NpcProgressBar>();
        let bar = q.single(world).expect("one bar");
        assert_eq!(bar.npc_id, 150001);
        assert_eq!(bar.target, player);
        assert_eq!(app.world().resource::<ActiveProgressBar>().npc_id, 150001);
    }

    #[test]
    fn completion_acks_progress_and_clears_gate() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<RespondToNpc>();
        app.insert_resource(ActiveProgressBar { npc_id: 42 });

        let target = app.world_mut().spawn_empty().id();
        let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
        timer.set_elapsed(std::time::Duration::from_secs(1));
        app.world_mut().spawn(NpcProgressBar {
            target,
            npc_id: 42,
            timer,
        });
        app.add_systems(Update, complete_progress_bar);
        app.update();

        let acks = app.world().resource::<Messages<RespondToNpc>>();
        let sent: Vec<_> = acks.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].npc_id, 42);
        assert_eq!(sent[0].response, NpcResponse::Progress);
        assert!(app.world().get_resource::<ActiveProgressBar>().is_none());
        let world = app.world_mut();
        assert_eq!(world.query::<&NpcProgressBar>().iter(world).count(), 0);
    }

    #[test]
    fn escape_acks_cancel_and_clears_gate() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<RespondToNpc>();
        app.add_message::<MoveRequested>();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        app.insert_resource(keys);
        app.insert_resource(ActiveProgressBar { npc_id: 9 });

        let target = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(NpcProgressBar {
            target,
            npc_id: 9,
            timer: Timer::from_seconds(5.0, TimerMode::Once),
        });
        app.add_systems(Update, cancel_progress_bar);
        app.update();

        let acks = app.world().resource::<Messages<RespondToNpc>>();
        let sent: Vec<_> = acks.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].npc_id, 9);
        assert_eq!(sent[0].response, NpcResponse::Cancel);
        assert!(app.world().get_resource::<ActiveProgressBar>().is_none());
        let world = app.world_mut();
        assert_eq!(world.query::<&NpcProgressBar>().iter(world).count(), 0);
    }

    #[test]
    fn movement_acks_cancel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<RespondToNpc>();
        app.add_message::<MoveRequested>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(ActiveProgressBar { npc_id: 3 });

        let target = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(NpcProgressBar {
            target,
            npc_id: 3,
            timer: Timer::from_seconds(5.0, TimerMode::Once),
        });
        app.world_mut()
            .resource_mut::<Messages<MoveRequested>>()
            .write(MoveRequested {
                dest_x: 10,
                dest_y: 20,
            });
        app.add_systems(Update, cancel_progress_bar);
        app.update();

        let acks = app.world().resource::<Messages<RespondToNpc>>();
        let sent: Vec<_> = acks.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].response, NpcResponse::Cancel);
    }
}
