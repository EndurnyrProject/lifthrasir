use super::{resource::TradeSession, systems};
use crate::core::state::GameState;
use bevy::prelude::*;

pub struct TradePlugin;

impl Plugin for TradePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TradeSession>()
            .add_systems(
                Update,
                (
                    systems::apply_trade_opened,
                    systems::apply_trade_offer.after(systems::apply_trade_opened),
                    systems::apply_trade_confirm_sent.after(systems::apply_trade_offer),
                    systems::apply_trade_ended.after(systems::apply_trade_confirm_sent),
                ),
            )
            .add_systems(OnExit(GameState::InGame), systems::reset_trade);
    }
}
