use super::resource::TradeSession;
use bevy::prelude::*;
use net_contract::commands::ConfirmTrade;
use net_contract::events::{TradeCancelled, TradeCompleted, TradeOfferUpdated, TradeOpened};

pub fn apply_trade_opened(
    mut opened: MessageReader<TradeOpened>,
    mut session: ResMut<TradeSession>,
) {
    for event in opened.read() {
        session.open(event.partner_char_id, event.partner_name.clone());
    }
}

pub fn apply_trade_offer(
    mut offers: MessageReader<TradeOfferUpdated>,
    mut session: ResMut<TradeSession>,
) {
    for offer in offers.read() {
        session.apply_offer(offer);
    }
}

pub fn apply_trade_confirm_sent(
    mut confirms: MessageReader<ConfirmTrade>,
    mut session: ResMut<TradeSession>,
) {
    for _ in confirms.read() {
        session.mark_confirm_sent();
    }
}

pub fn apply_trade_ended(
    mut completed: MessageReader<TradeCompleted>,
    mut cancelled: MessageReader<TradeCancelled>,
    mut session: ResMut<TradeSession>,
) {
    if completed.read().count() + cancelled.read().count() > 0 {
        session.close();
    }
}

pub fn reset_trade(mut session: ResMut<TradeSession>) {
    session.reset();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::state::GameState, domain::trade::TradePlugin};
    use bevy::state::app::StatesPlugin;
    use net_contract::dto::TradeCancelReason;

    fn app_with_trade() -> App {
        let mut app = App::new();
        app.add_message::<TradeOpened>()
            .add_message::<TradeOfferUpdated>()
            .add_message::<TradeCancelled>()
            .add_message::<TradeCompleted>()
            .add_message::<ConfirmTrade>()
            .add_plugins(TradePlugin);
        app
    }

    fn offer() -> TradeOfferUpdated {
        TradeOfferUpdated {
            own: vec![],
            partner: vec![],
            own_zeny: 42,
            partner_zeny: 33,
            own_locked: true,
            partner_locked: true,
        }
    }

    #[test]
    fn opened_is_applied_before_offer_in_same_frame() {
        let mut app = app_with_trade();
        app.world_mut().write_message(TradeOpened {
            partner_char_id: 7,
            partner_name: "Alice".into(),
        });
        app.world_mut().write_message(offer());
        app.update();
        let open = app.world().resource::<TradeSession>().current().unwrap();
        assert_eq!(
            (open.partner_char_id, open.own_zeny, open.partner_zeny),
            (7, 42, 33)
        );
        assert!(open.own_locked && open.partner_locked);
        app.world_mut().write_message(ConfirmTrade);
        app.update();
        assert!(
            app.world()
                .resource::<TradeSession>()
                .current()
                .unwrap()
                .confirm_sent
        );
    }

    #[test]
    fn both_ending_events_close_but_offer_before_open_is_ignored() {
        let mut app = app_with_trade();
        app.world_mut().write_message(offer());
        app.update();
        assert!(!app.world().resource::<TradeSession>().is_open());
        for reason in [Some(TradeCancelReason::Cancelled), None] {
            app.world_mut().write_message(TradeOpened {
                partner_char_id: 7,
                partner_name: "Alice".into(),
            });
            app.update();
            assert!(app.world().resource::<TradeSession>().is_open());
            if let Some(reason) = reason {
                app.world_mut().write_message(TradeCancelled { reason });
            } else {
                app.world_mut().write_message(TradeCompleted);
            }
            app.update();
            assert!(!app.world().resource::<TradeSession>().is_open());
        }
    }

    #[test]
    fn leaving_in_game_resets_session() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin).init_state::<GameState>();
        app.add_message::<TradeOpened>()
            .add_message::<TradeOfferUpdated>()
            .add_message::<TradeCancelled>()
            .add_message::<TradeCompleted>()
            .add_message::<ConfirmTrade>()
            .add_plugins(TradePlugin);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.world_mut().write_message(TradeOpened {
            partner_char_id: 7,
            partner_name: "Alice".into(),
        });
        app.update();
        assert!(app.world().resource::<TradeSession>().is_open());
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Login);
        app.update();
        assert!(!app.world().resource::<TradeSession>().is_open());
    }
}
