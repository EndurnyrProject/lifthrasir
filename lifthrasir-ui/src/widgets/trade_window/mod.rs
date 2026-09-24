use bevy::prelude::*;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;

use crate::theme::feathers_theme::install_norse_theme;

pub mod feedback;
pub mod request_dialog;
pub mod slash;

pub use request_dialog::PendingTradeRequest;
pub use slash::TradeSlashSubmitted;

pub struct TradeWindowPlugin;

impl Plugin for TradeWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<PendingTradeRequest>()
            .add_message::<TradeSlashSubmitted>()
            .add_systems(
                Update,
                (
                    request_dialog::show_incoming_request,
                    request_dialog::claim_request_choice,
                    request_dialog::expire_pending_request,
                    request_dialog::clear_on_trade_opened,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                feedback::ingest_trade_feedback.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                slash::dispatch_trade_slash.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                request_dialog::reset_pending_request,
            );
    }
}
