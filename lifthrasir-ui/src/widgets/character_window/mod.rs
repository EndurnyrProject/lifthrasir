//! Unified character window (Console): one draggable BSN/Feathers window with an
//! identity strip and three top tabs — Character, Bag, Skills — replacing the four
//! separate in-game windows.
//!
//! This module owns the open/tab state machine ([`CharacterWindowState`] +
//! [`next_state`]), the four-chord toggle ([`toggle_character_window`]), and the
//! visibility projection ([`reflect_window_state`]). The persistent chrome is built
//! in [`shell`]. The plugin is intentionally left unregistered while the four old
//! windows still exist (architecture §5.5); it stays inert until the integration
//! task wires it and deletes the old windows.

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use leafwing_input_manager::prelude::ActionState;

use crate::theme::feathers_theme::install_norse_theme;

mod bag_tab;
mod character_tab;
mod identity;
mod meter;
pub mod shell;
mod skills_tab;

/// Which tab the Console shows. The active chord always selects the tab, so this is
/// never "remembered" independently of the last chord.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CharacterTab {
    #[default]
    Character,
    Bag,
    Skills,
}

/// Console open/tab state. Default: closed, on the Character tab.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CharacterWindowState {
    pub open: bool,
    pub tab: CharacterTab,
}

/// The Console root; the toggle/reflect/drag/close systems find the window by it.
#[derive(Component, Default, Clone)]
pub struct CharacterWindowRoot;

/// The draggable titlebar drag handle.
#[derive(Component, Default, Clone)]
pub struct CharacterTitlebar;

/// The identity-strip mount; Task 3 fills its children.
#[derive(Component, Default, Clone)]
pub struct CharacterIdentityMount;

/// The Character tab body; Task 5 fills its children.
#[derive(Component, Default, Clone)]
pub struct CharacterTabBody;

/// The Bag tab body; Task 4 fills its children.
#[derive(Component, Default, Clone)]
pub struct BagTabBody;

/// The Skills tab body; Task 6 fills its children.
#[derive(Component, Default, Clone)]
pub struct SkillsTabBody;

/// Marks a tab-strip button with the tab it selects.
#[derive(Component, Clone, Copy, Default)]
pub struct CharacterTabButton(pub CharacterTab);

/// Pure toggle state machine: a chord opens the Console on its tab; the same chord
/// while open on that tab closes it; a different chord switches tab and stays open.
pub fn next_state(cur: CharacterWindowState, chord_tab: CharacterTab) -> CharacterWindowState {
    if !cur.open {
        CharacterWindowState {
            open: true,
            tab: chord_tab,
        }
    } else if cur.tab == chord_tab {
        CharacterWindowState {
            open: false,
            tab: cur.tab,
        }
    } else {
        CharacterWindowState {
            open: true,
            tab: chord_tab,
        }
    }
}

pub struct CharacterWindowPlugin;

impl Plugin for CharacterWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<CharacterWindowState>();
        app.init_resource::<bag_tab::BagUi>();
        app.init_resource::<bag_tab::LastBagClick>();
        app.init_resource::<skills_tab::SkillPanelUi>();
        app.init_resource::<skills_tab::SkillPanelStaging>();
        app.init_resource::<skills_tab::LastSkillPanelClick>();
        character_tab::register(app);
        app.add_systems(
            Update,
            toggle_character_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            bag_tab::rebuild_bag_body.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            (
                skills_tab::ensure_default_tab,
                skills_tab::rebuild_skills_body,
                skills_tab::update_skill_footer,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            OnExit(GameState::InGame),
            (bag_tab::reset, skills_tab::reset),
        );
        app.add_systems(
            Update,
            reflect_window_state.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            identity::rebuild_identity
                .run_if(in_state(GameState::InGame).and_then(identity::identity_changed)),
        );
    }
}

/// Maps a just-pressed chord to its tab: Status/Equipment → Character, Inventory →
/// Bag, Skills → Skills. First match wins if several fire the same frame.
fn chord_tab(actions: &ActionState<PlayerAction>) -> Option<CharacterTab> {
    if actions.just_pressed(&PlayerAction::Status) || actions.just_pressed(&PlayerAction::Equipment)
    {
        Some(CharacterTab::Character)
    } else if actions.just_pressed(&PlayerAction::Inventory) {
        Some(CharacterTab::Bag)
    } else if actions.just_pressed(&PlayerAction::Skills) {
        Some(CharacterTab::Skills)
    } else {
        None
    }
}

/// The four chords drive the Console open/tab state through [`next_state`].
fn toggle_character_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut state: ResMut<CharacterWindowState>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    let Some(tab) = chord_tab(actions) else {
        return;
    };
    *state = next_state(*state, tab);
}

/// Sets the single entity matched by `q` visible or hidden, writing only on change.
fn set_visible<F: QueryFilter>(q: &mut Query<&mut Visibility, F>, visible: bool) {
    let Ok(mut vis) = q.single_mut() else {
        return;
    };
    let want = if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if *vis != want {
        *vis = want;
    }
}

/// The four disjoint `&mut Visibility` queries `reflect_window_state` drives; the
/// `Without` filters keep them provably non-overlapping so the system builds.
type RootVis<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CharacterWindowRoot>,
        Without<CharacterTabBody>,
        Without<BagTabBody>,
        Without<SkillsTabBody>,
    ),
>;
type CharBodyVis<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CharacterTabBody>,
        Without<BagTabBody>,
        Without<SkillsTabBody>,
    ),
>;
type BagBodyVis<'w, 's> =
    Query<'w, 's, &'static mut Visibility, (With<BagTabBody>, Without<SkillsTabBody>)>;
type SkillsBodyVis<'w, 's> = Query<'w, 's, &'static mut Visibility, With<SkillsTabBody>>;

/// Projects [`CharacterWindowState`] onto visibility: `open` → root, `tab` → the
/// active tab body.
fn reflect_window_state(
    state: Res<CharacterWindowState>,
    mut root: RootVis,
    mut character: CharBodyVis,
    mut bag: BagBodyVis,
    mut skills: SkillsBodyVis,
) {
    set_visible(&mut root, state.open);
    set_visible(&mut character, state.tab == CharacterTab::Character);
    set_visible(&mut bag, state.tab == CharacterTab::Bag);
    set_visible(&mut skills, state.tab == CharacterTab::Skills);
}

/// A tab-strip button click sets the active tab (never toggles closed).
fn on_tab_click(
    click: On<Pointer<Click>>,
    buttons: Query<&CharacterTabButton>,
    mut state: ResMut<CharacterWindowState>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    state.tab = button.0;
}

/// The titlebar close button clears `open` on the resource — the single source of
/// truth `reflect_window_state` reads. Hiding root `Visibility` directly would be
/// undone the next frame, so the Console does not use `chrome::close_window`.
fn on_close_click(_: On<Activate>, mut state: ResMut<CharacterWindowState>) {
    state.open = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_opens_on_chord_tab() {
        let cur = CharacterWindowState::default();
        assert_eq!(
            next_state(cur, CharacterTab::Bag),
            CharacterWindowState {
                open: true,
                tab: CharacterTab::Bag,
            }
        );
    }

    #[test]
    fn open_same_tab_closes() {
        let cur = CharacterWindowState {
            open: true,
            tab: CharacterTab::Skills,
        };
        assert_eq!(
            next_state(cur, CharacterTab::Skills),
            CharacterWindowState {
                open: false,
                tab: CharacterTab::Skills,
            }
        );
    }

    #[test]
    fn close_button_clears_open_on_the_resource() {
        let mut app = App::new();
        app.insert_resource(CharacterWindowState {
            open: true,
            tab: CharacterTab::Bag,
        });
        let button = app.world_mut().spawn_empty().observe(on_close_click).id();
        app.world_mut().trigger(Activate { entity: button });

        let state = app.world().resource::<CharacterWindowState>();
        assert!(!state.open);
        assert_eq!(state.tab, CharacterTab::Bag);
    }

    #[test]
    fn open_different_tab_switches_and_stays_open() {
        let cur = CharacterWindowState {
            open: true,
            tab: CharacterTab::Character,
        };
        assert_eq!(
            next_state(cur, CharacterTab::Bag),
            CharacterWindowState {
                open: true,
                tab: CharacterTab::Bag,
            }
        );
    }
}
