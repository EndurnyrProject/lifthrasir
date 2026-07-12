use bevy::prelude::*;

use super::{assets, render, send};
use crate::core::state::GameState;

pub struct EmotePlugin;

impl Plugin for EmotePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<send::EmoteCooldown>()
            .add_message::<send::EmoteRequested>()
            .add_systems(OnEnter(GameState::InGame), assets::load_emote_assets)
            .add_systems(
                Update,
                (
                    assets::finalize_emote_assets,
                    render::spawn_emote,
                    render::advance_and_despawn_emotes,
                    send::tick_emote_cooldown,
                    send::handle_emote_request,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
