use super::{emblems, resource::GuildState, systems};
use crate::core::state::GameState;
use bevy::prelude::*;
use net_contract::state::ZoneSessionGeneration;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuildSystems {
    SessionReset,
    Apply,
    EmblemReceive,
    UiSync,
    EmblemSend,
}

#[derive(Resource, Default)]
pub(super) struct GuildSessionGate {
    pub generation: ZoneSessionGeneration,
    pub char_id: u32,
    pub blocked: bool,
}

pub struct GuildPlugin;

impl Plugin for GuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuildState>()
            .init_resource::<GuildSessionGate>()
            .init_resource::<emblems::GuildEmblemImages>()
            .init_resource::<Assets<Image>>()
            .add_message::<net_contract::commands::GuildEmblemFetchRequested>()
            .configure_sets(
                Update,
                (
                    GuildSystems::SessionReset,
                    GuildSystems::Apply,
                    GuildSystems::EmblemReceive,
                    GuildSystems::UiSync,
                    GuildSystems::EmblemSend,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (systems::reset_guild_session, emblems::reset_emblems)
                    .chain()
                    .in_set(GuildSystems::SessionReset),
            )
            .add_systems(
                Update,
                systems::apply_guild_ingress.in_set(GuildSystems::Apply),
            )
            .add_systems(
                Update,
                emblems::receive_emblem_data.in_set(GuildSystems::EmblemReceive),
            )
            .add_systems(
                Update,
                emblems::send_next_fetch.in_set(GuildSystems::EmblemSend),
            )
            .add_systems(
                OnEnter(GameState::CharacterSelection),
                (
                    systems::block_guild_on_character_select,
                    emblems::block_emblems,
                )
                    .chain(),
            );
    }
}
