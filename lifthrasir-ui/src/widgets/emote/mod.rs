//! Emote UI: the chat slash parser ([`slash`]) and the toggleable thumbnail picker
//! ([`scene`]).
//!
//! The picker is authored as a BSN scene: a titled panel over a flex-wrap grid of one
//! `@FeathersButton` cell per emote id. Clicking a cell writes `EmoteRequested`; the
//! game-engine cooldown gate decides whether it becomes a wire send. Two systems keep
//! the grid live: [`populate_emote_thumbnails`] fills each cell's image once the shared
//! `EmoteAssets` resource loads, and [`refresh_emote_cooldown`] greys the cells (via
//! `InteractionDisabled`) while the client cooldown is unfinished.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::emote::assets::EmoteAssets;
use game_engine::domain::emote::EmoteCooldown;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use leafwing_input_manager::prelude::ActionState;

use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;
pub mod slash;

pub use scene::build as spawn_emote_picker;

/// Marks the picker-window root so the toggle and close systems can flip its
/// `Visibility`.
#[derive(Component, Default, Clone)]
pub struct EmotePickerRoot;

/// One grid cell, tagged with the emote id it sends. `populate_emote_thumbnails` reads
/// the id to pick a thumbnail; `on_emote_click` reads it to write the intent.
#[derive(Component, Default, Clone, Copy)]
pub struct EmoteButton(pub u32);

pub struct EmotePickerPlugin;

impl Plugin for EmotePickerPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(
            Update,
            toggle_emote_picker.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                populate_emote_thumbnails.run_if(resource_exists::<EmoteAssets>),
                refresh_emote_cooldown,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// `PlayerAction::Emote` (Alt+M) toggles the picker's visibility.
pub fn toggle_emote_picker(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<EmotePickerRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Emote) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

/// Fill each cell's caption `ImageNode` from `EmoteAssets.thumbnails[id]`. Runs while
/// the resource exists rather than once on add, so it also re-fills a HUD respawned on
/// re-entering gameplay. Idempotent: the handle is written only when it differs, so a
/// settled grid is a no-op with no change-detection churn.
pub fn populate_emote_thumbnails(
    assets: Res<EmoteAssets>,
    buttons: Query<(&EmoteButton, &Children)>,
    mut images: Query<&mut ImageNode>,
) {
    for (button, children) in &buttons {
        let Some(thumbnail) = assets.thumbnails.get(button.0 as usize) else {
            continue;
        };
        for &child in children {
            let Ok(mut image) = images.get_mut(child) else {
                continue;
            };
            if image.image != *thumbnail {
                image.image = thumbnail.clone();
            }
        }
    }
}

/// Grey the cells while the client cooldown is unfinished by toggling Feathers'
/// `InteractionDisabled` on every [`EmoteButton`] (which both dims the button and
/// suppresses its `Activate`). Edge-gated on a `Local` so the 88 entities are only
/// touched on a ready/not-ready transition, not every frame.
pub fn refresh_emote_cooldown(
    cooldown: Res<EmoteCooldown>,
    buttons: Query<Entity, With<EmoteButton>>,
    mut commands: Commands,
    mut was_disabled: Local<Option<bool>>,
) {
    let disabled = !cooldown.0.is_finished();
    if *was_disabled == Some(disabled) {
        return;
    }
    *was_disabled = Some(disabled);
    for entity in &buttons {
        if disabled {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
}
