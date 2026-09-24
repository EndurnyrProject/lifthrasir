use bevy::prelude::*;
use net_contract::commands::RequestTrade;
use net_contract::dto::TradeCancelReason;
use net_contract::events::{TradeCancelled, TradeCompleted};

use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

pub fn cancel_reason_text(reason: TradeCancelReason) -> &'static str {
    match reason {
        TradeCancelReason::Declined => "The trade request was declined.",
        TradeCancelReason::Timeout => "The trade request timed out.",
        TradeCancelReason::Cancelled => "Trade cancelled.",
        TradeCancelReason::TooFar => "The other player is too far away to trade.",
        TradeCancelReason::Busy => "The other player is busy.",
        TradeCancelReason::Dead => "The trade ended because a player died.",
        TradeCancelReason::Disconnected => "The trade ended because a player disconnected.",
        TradeCancelReason::Capacity => "The trade failed: not enough inventory space or weight.",
        TradeCancelReason::Invalid => "The trade could not be completed.",
        TradeCancelReason::Unknown(_) => "Trade cancelled.",
    }
}

pub(crate) fn ingest_trade_feedback(
    mut completed: MessageReader<TradeCompleted>,
    mut cancelled: MessageReader<TradeCancelled>,
    mut requested: MessageReader<RequestTrade>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if completed.is_empty() && cancelled.is_empty() && requested.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);
    for _ in completed.read() {
        append_colored_line(
            &mut commands,
            container,
            "Trade completed.",
            theme::EMERALD,
            font.clone(),
        );
    }
    for event in cancelled.read() {
        if let TradeCancelReason::Unknown(raw) = event.reason {
            warn!(raw, "unknown trade cancellation reason");
        }
        append_colored_line(
            &mut commands,
            container,
            cancel_reason_text(event.reason),
            theme::BAD,
            font.clone(),
        );
    }
    for _ in requested.read() {
        append_colored_line(
            &mut commands,
            container,
            "Trade request sent.",
            theme::WARN,
            font.clone(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cancellation_reasons_have_distinct_readable_messages() {
        let known = [
            TradeCancelReason::Declined,
            TradeCancelReason::Timeout,
            TradeCancelReason::Cancelled,
            TradeCancelReason::TooFar,
            TradeCancelReason::Busy,
            TradeCancelReason::Dead,
            TradeCancelReason::Disconnected,
            TradeCancelReason::Capacity,
            TradeCancelReason::Invalid,
        ];
        let lines: HashSet<_> = known.into_iter().map(cancel_reason_text).collect();
        assert_eq!(lines.len(), 9);
        assert_eq!(
            cancel_reason_text(TradeCancelReason::Unknown(999)),
            "Trade cancelled."
        );
    }

    #[test]
    fn each_event_appends_one_line_and_no_duplicate_next_frame() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<TradeCompleted>()
            .add_message::<TradeCancelled>()
            .add_message::<RequestTrade>();
        app.world_mut().spawn(ChatHistory);
        app.add_systems(Update, ingest_trade_feedback);
        app.world_mut().write_message(TradeCompleted);
        app.world_mut().write_message(TradeCancelled {
            reason: TradeCancelReason::Busy,
        });
        app.world_mut()
            .write_message(RequestTrade { target_char_id: 7 });
        app.update();
        let mut lines = app.world_mut().query::<(&Text, &TextColor)>();
        let rendered: Vec<_> = lines
            .iter(app.world())
            .map(|(text, color)| (text.0.clone(), color.0))
            .collect();
        assert_eq!(rendered.len(), 3);
        assert!(rendered.contains(&("Trade completed.".into(), theme::EMERALD)));
        assert!(rendered.contains(&("The other player is busy.".into(), theme::BAD)));
        assert!(rendered.contains(&("Trade request sent.".into(), theme::WARN)));
        app.update();
        assert_eq!(lines.iter(app.world()).count(), 3);
    }
}
