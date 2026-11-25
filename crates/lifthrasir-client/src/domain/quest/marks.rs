//! Quest marks: the server's per-NPC quest icons for the current map.
//!
//! The server sends icons only when the map name changes, so the marks survive a
//! same-map warp and are dropped when a warp names a different map.

use std::collections::BTreeMap;

use bevy::prelude::*;
use lifthrasir_assets::paths::quest_mark_icon_path;
use net_contract::events::{MapChangeRequested, QuestMarkChanged};

use crate::core::state::GameState;
use crate::domain::world::spawn_context::MapSpawnContext;

/// One NPC's quest mark: the cell it stands on and the retail icon and color ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestMark {
    pub x: u16,
    pub y: u16,
    pub icon: u32,
    pub color: u32,
}

/// The quest marks of the current map, one per NPC id, and the map they belong to.
#[derive(Resource, Default, Debug)]
pub struct QuestMarks {
    marks: BTreeMap<u32, QuestMark>,
    map: Option<String>,
}

impl QuestMarks {
    /// Replays one server event. Unknown icon or color values are kept and logged;
    /// the renderers skip what they cannot draw.
    pub fn apply(&mut self, event: &QuestMarkChanged) {
        match *event {
            QuestMarkChanged::Show {
                npc_id,
                x,
                y,
                icon,
                color,
            } => {
                if quest_mark_icon_path(icon, color).is_none() {
                    warn!("quest mark for npc {npc_id}: unknown icon {icon} or color {color}");
                }
                self.marks.insert(npc_id, QuestMark { x, y, icon, color });
            }
            QuestMarkChanged::Clear { npc_id } => {
                self.marks.remove(&npc_id);
            }
        }
    }

    pub fn get(&self, npc_id: u32) -> Option<&QuestMark> {
        self.marks.get(&npc_id)
    }

    /// The marks in NPC id order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &QuestMark)> {
        self.marks.iter().map(|(npc_id, mark)| (*npc_id, mark))
    }
}

/// Replays the server's quest mark events while a map session is active
/// ([`GameState::Loading`] or [`GameState::InGame`]), so icons sent while the map
/// is still loading are kept. In any other state the events are drained and
/// dropped, so an old session's icons never reach the next character's map.
/// Before the first warp of the session, the marks are recorded on
/// [`MapSpawnContext`]'s map; after that, the warp owns the recorded map, so a
/// stale context never overwrites it.
pub(super) fn replay_quest_marks(
    mut events: MessageReader<QuestMarkChanged>,
    state: Res<State<GameState>>,
    ctx: Option<Res<MapSpawnContext>>,
    mut marks: ResMut<QuestMarks>,
) {
    if !matches!(state.get(), GameState::Loading | GameState::InGame) {
        events.clear();
        return;
    }
    if events.is_empty() {
        return;
    }
    for event in events.read() {
        marks.apply(event);
    }
    if marks.map.is_none() {
        marks.map = ctx.map(|ctx| ctx.map_name.clone());
    }
}

/// Drops the marks when a warp names a map other than the one they were recorded
/// on, then records the warp's destination. Independent of system order: it never
/// reads [`MapSpawnContext`], which `handle_map_change` overwrites. Runs with
/// [`replay_quest_marks`] in `PostUpdate`, after the adapters drain in `Update`,
/// so a warp and the icons that follow it in one frame are seen together.
pub(super) fn clear_quest_marks_on_map_change(
    mut requests: MessageReader<MapChangeRequested>,
    mut marks: ResMut<QuestMarks>,
) {
    for request in requests.read() {
        if marks.map.as_deref() != Some(request.map_name.as_str()) {
            marks.marks.clear();
        }
        marks.map = Some(request.map_name.clone());
    }
}

pub(super) fn reset_quest_marks(mut marks: ResMut<QuestMarks>) {
    *marks = QuestMarks::default();
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;
    use net_contract::events::QuestEvent;

    use super::*;
    use crate::domain::quest::QuestPlugin;

    fn show(npc_id: u32, icon: u32, color: u32) -> QuestMarkChanged {
        QuestMarkChanged::Show {
            npc_id,
            x: 150,
            y: 180,
            icon,
            color,
        }
    }

    fn warp(map_name: &str) -> MapChangeRequested {
        MapChangeRequested {
            map_name: map_name.into(),
            x: 10,
            y: 20,
        }
    }

    fn app_in(state: GameState) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(state)
            .add_message::<QuestEvent>()
            .add_message::<QuestMarkChanged>()
            .add_message::<MapChangeRequested>()
            .add_plugins(QuestPlugin);
        app
    }

    fn app_on_prontera_with_mark() -> App {
        let mut app = app_in(GameState::Loading);
        app.insert_resource(MapSpawnContext::new("prontera".into(), 150, 150, 1));
        app.world_mut().write_message(show(7, 1, 1));
        app.update();
        assert!(app.world().resource::<QuestMarks>().get(7).is_some());
        app
    }

    fn npc_ids(app: &App) -> Vec<u32> {
        app.world()
            .resource::<QuestMarks>()
            .iter()
            .map(|(npc_id, _)| npc_id)
            .collect()
    }

    #[test]
    fn show_inserts_a_mark() {
        let mut marks = QuestMarks::default();
        marks.apply(&show(7, 1, 2));
        assert_eq!(
            marks.get(7),
            Some(&QuestMark {
                x: 150,
                y: 180,
                icon: 1,
                color: 2
            })
        );
    }

    #[test]
    fn show_replaces_the_npc_mark() {
        let mut marks = QuestMarks::default();
        marks.apply(&show(7, 1, 2));
        marks.apply(&show(7, 5, 3));
        assert_eq!(
            marks.get(7).map(|mark| (mark.icon, mark.color)),
            Some((5, 3))
        );
        assert_eq!(marks.iter().count(), 1);
    }

    #[test]
    fn clear_removes_the_mark() {
        let mut marks = QuestMarks::default();
        marks.apply(&show(7, 1, 2));
        marks.apply(&show(8, 1, 2));
        marks.apply(&QuestMarkChanged::Clear { npc_id: 7 });
        assert!(marks.get(7).is_none());
        assert!(marks.get(8).is_some());
    }

    #[test]
    fn clear_for_unknown_npc_is_a_no_op() {
        let mut marks = QuestMarks::default();
        marks.apply(&show(7, 1, 2));
        marks.apply(&QuestMarkChanged::Clear { npc_id: 99 });
        assert_eq!(
            marks.iter().map(|(npc_id, _)| npc_id).collect::<Vec<_>>(),
            [7]
        );
    }

    #[test]
    fn unknown_icon_or_color_is_stored() {
        let mut marks = QuestMarks::default();
        marks.apply(&show(7, 11, 4));
        assert_eq!(
            marks.get(7).map(|mark| (mark.icon, mark.color)),
            Some((11, 4))
        );
    }

    #[test]
    fn marks_arriving_outside_in_game_are_kept() {
        let mut app = app_in(GameState::Loading);
        app.world_mut().write_message(show(7, 1, 1));
        app.update();
        assert_eq!(npc_ids(&app), [7]);
    }

    #[test]
    fn warp_to_another_map_clears_marks() {
        let mut app = app_on_prontera_with_mark();
        app.world_mut().write_message(warp("geffen"));
        app.update();
        assert!(npc_ids(&app).is_empty());
    }

    #[test]
    fn warp_to_the_same_map_keeps_marks() {
        let mut app = app_on_prontera_with_mark();
        app.world_mut().write_message(warp("prontera"));
        app.update();
        assert_eq!(npc_ids(&app), [7]);
    }

    #[test]
    fn clear_survives_the_context_already_switching() {
        let mut app = app_on_prontera_with_mark();
        app.world_mut().resource_mut::<MapSpawnContext>().map_name = "geffen".into();
        app.world_mut().write_message(warp("geffen"));
        app.update();
        assert!(npc_ids(&app).is_empty());
    }

    #[test]
    fn new_icons_in_the_warp_frame_survive_the_clear() {
        let mut app = app_on_prontera_with_mark();
        app.world_mut().write_message(warp("geffen"));
        app.world_mut().write_message(show(9, 2, 1));
        app.update();
        assert_eq!(npc_ids(&app), [9]);
        app.world_mut().write_message(warp("geffen"));
        app.update();
        assert_eq!(npc_ids(&app), [9]);
    }

    #[test]
    fn warp_and_icon_written_during_update_survive_the_next_frame() {
        let mut app = app_on_prontera_with_mark();
        app.add_systems(
            Update,
            (
                (|mut writer: MessageWriter<MapChangeRequested>| {
                    writer.write(warp("geffen"));
                })
                .run_if(run_once)
                .after(clear_quest_marks_on_map_change),
                (|mut writer: MessageWriter<QuestMarkChanged>| {
                    writer.write(show(9, 2, 1));
                })
                .run_if(run_once)
                .before(replay_quest_marks),
            ),
        );
        app.update();
        app.update();
        assert_eq!(npc_ids(&app), [9]);
    }

    #[test]
    fn marks_arriving_outside_the_map_session_are_dropped() {
        let mut app = app_in(GameState::CharacterSelection);
        app.world_mut().write_message(show(7, 1, 1));
        app.update();
        assert!(npc_ids(&app).is_empty());
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();
        assert!(npc_ids(&app).is_empty());
    }

    #[test]
    fn character_select_resets_marks() {
        let mut app = app_on_prontera_with_mark();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();
        assert!(npc_ids(&app).is_empty());
    }
}
