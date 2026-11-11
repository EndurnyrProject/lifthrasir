//! The Character window: the hero's header (name, class, levels and meters), the
//! equipment sets either side of the live preview, and the stat allocation and combat
//! read-outs. Sizes follow the `Character` artboard of the Lifthrasir Widgets design.
//!
//! [`shell`] spawns the frame once. [`identity`] and [`equip`] respawn their mounts
//! from view-models on change, [`attributes`] patches its marked texts in place and
//! [`preview`] owns the render-to-texture rig. Skills live in their own window
//! ([`crate::widgets::skills_window`]), the bag in
//! [`crate::widgets::inventory_window`].

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::{PlayerAction, ui_unfocused};

mod attributes;
mod equip;
mod identity;
mod preview;
pub mod shell;

/// Whether the window is shown. The single source of truth [`reflect_window_state`]
/// projects onto the root.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CharacterWindowState {
    pub open: bool,
}

#[derive(Component, Default, Clone)]
pub struct CharacterWindowRoot;

/// The title bar, which is the drag handle.
#[derive(Component, Default, Clone)]
pub struct CharacterTitlebar;

/// Mount of the header: name, class, levels and meters.
#[derive(Component, Default, Clone)]
pub struct CharacterIdentityMount;

pub struct CharacterWindowPlugin;

impl Plugin for CharacterWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterWindowState>();
        attributes::register(app);
        equip::register(app);
        preview::register(app);
        app.add_systems(
            Update,
            (
                toggle_character_window.run_if(ui_unfocused),
                reflect_window_state,
                identity::rebuild_identity.run_if(identity::identity_changed),
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// The Status and Equipment chords both show or hide the window.
fn toggle_character_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut state: ResMut<CharacterWindowState>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if actions.just_pressed(&PlayerAction::Status) || actions.just_pressed(&PlayerAction::Equipment)
    {
        state.open = !state.open;
    }
}

fn reflect_window_state(
    state: Res<CharacterWindowState>,
    mut roots: Query<&mut Visibility, With<CharacterWindowRoot>>,
) {
    let want = if state.open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut roots {
        visibility.set_if_neq(want);
    }
}

/// The close button clears `open` on the resource. Hiding the root directly would be
/// undone by [`reflect_window_state`] the next frame.
fn on_close_click(_: On<Activate>, mut state: ResMut<CharacterWindowState>) {
    state.open = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_button_clears_open_on_the_resource() {
        let mut app = App::new();
        app.insert_resource(CharacterWindowState { open: true });
        let button = app.world_mut().spawn_empty().observe(on_close_click).id();
        app.world_mut().trigger(Activate { entity: button });

        assert!(!app.world().resource::<CharacterWindowState>().open);
    }

    #[test]
    fn open_state_drives_the_root_visibility() {
        let mut app = App::new();
        app.insert_resource(CharacterWindowState { open: true });
        app.add_systems(Update, reflect_window_state);
        let root = app
            .world_mut()
            .spawn((CharacterWindowRoot, Visibility::Hidden))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Visible)
        );

        app.world_mut().resource_mut::<CharacterWindowState>().open = false;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Hidden)
        );
    }
}
