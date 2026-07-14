use super::{resource::GuildState, systems};
use crate::core::state::GameState;
use bevy::prelude::*;
use net_contract::state::ZoneSessionGeneration;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuildSystems {
    SessionReset,
    Apply,
    UiSync,
}

#[derive(Resource, Default)]
pub(super) struct GuildSessionGate {
    pub generation: ZoneSessionGeneration,
    pub blocked: bool,
}

pub struct GuildPlugin;

impl Plugin for GuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuildState>()
            .init_resource::<GuildSessionGate>()
            .configure_sets(
                Update,
                (
                    GuildSystems::SessionReset,
                    GuildSystems::Apply,
                    GuildSystems::UiSync,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                systems::reset_guild_session.in_set(GuildSystems::SessionReset),
            )
            .add_systems(
                Update,
                systems::apply_guild_ingress.in_set(GuildSystems::Apply),
            )
            .add_systems(
                OnEnter(GameState::CharacterSelection),
                systems::block_guild_on_character_select,
            );
    }
}
