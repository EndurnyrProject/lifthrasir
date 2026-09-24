use bevy::prelude::*;
use game_engine::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};
use net_contract::commands::RespondTrade;
use net_contract::events::{TradeOpened, TradeRequestNotified};

use crate::widgets::system_dialog::SystemDialogRoot;

/// Server invite timeout mirrored by the local request dialog.
pub const REQUEST_TTL_SECS: f32 = 30.0;

#[derive(Resource, Default)]
pub struct PendingTradeRequest {
    pub char_id: u32,
    pub name: String,
    timer: Timer,
    correlation: Option<u64>,
    next_correlation: u64,
}

impl PendingTradeRequest {
    pub fn is_pending(&self) -> bool {
        self.correlation.is_some()
    }

    fn set(&mut self, request: &TradeRequestNotified) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.char_id = request.char_id;
        self.name.clone_from(&request.name);
        self.timer = Timer::from_seconds(REQUEST_TTL_SECS, TimerMode::Once);
        self.correlation = Some(self.next_correlation);
    }

    pub(crate) fn clear(&mut self) {
        self.char_id = 0;
        self.name.clear();
        self.timer = Timer::default();
        self.correlation = None;
    }
}

/// Show the shared modal unless another dialog or request already owns it.
pub fn show_incoming_request(
    mut requests: MessageReader<TradeRequestNotified>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut pending: ResMut<PendingTradeRequest>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    if !existing.is_empty() || pending.is_pending() {
        return;
    }
    pending.set(request);
    dialogs.write(ShowSystemDialog {
        severity: DialogSeverity::Info,
        kind: SystemDialogKind::TradeRequest,
        kicker: "Trade".into(),
        title: "Trade Request".into(),
        message: format!("{} wants to trade with you.", request.name),
        code: String::new(),
        button_label: "Accept".into(),
        secondary_label: "Decline".into(),
        confirm_state: None,
        correlation: pending.correlation,
    });
}

/// Only the exact pending trade dialog can answer the request.
pub fn claim_request_choice(
    mut choices: MessageReader<SystemDialogChoice>,
    mut pending: ResMut<PendingTradeRequest>,
    mut responses: MessageWriter<RespondTrade>,
) {
    if !pending.is_pending() {
        return;
    }
    let Some(choice) = choices.read().find(|choice| {
        choice.kind == SystemDialogKind::TradeRequest && choice.correlation == pending.correlation
    }) else {
        return;
    };
    responses.write(RespondTrade {
        accept: choice.primary,
    });
    pending.clear();
}

/// Discard expired requests and despawn only their own dialog.
pub fn expire_pending_request(
    time: Res<Time>,
    mut pending: ResMut<PendingTradeRequest>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !pending.is_pending() || !pending.timer.tick(time.delta()).just_finished() {
        return;
    }
    let correlation = pending.correlation;
    pending.clear();
    if let Some((entity, _)) = roots
        .iter()
        .find(|(_, root)| root.matches(SystemDialogKind::TradeRequest, correlation))
    {
        commands.entity(entity).despawn();
    }
}

pub fn clear_on_trade_opened(
    mut opened: MessageReader<TradeOpened>,
    mut pending: ResMut<PendingTradeRequest>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if opened.read().next().is_none() || !pending.is_pending() {
        return;
    }
    let correlation = pending.correlation;
    pending.clear();
    if let Some((entity, _)) = roots
        .iter()
        .find(|(_, root)| root.matches(SystemDialogKind::TradeRequest, correlation))
    {
        commands.entity(entity).despawn();
    }
}

pub fn reset_pending_request(mut pending: ResMut<PendingTradeRequest>) {
    pending.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn request() -> TradeRequestNotified {
        TradeRequestNotified {
            char_id: 7,
            name: "Alice".into(),
        }
    }

    #[test]
    fn incoming_request_shows_correlated_dialog_and_drops_when_busy() {
        let mut app = App::new();
        app.add_message::<TradeRequestNotified>()
            .add_message::<ShowSystemDialog>()
            .init_resource::<PendingTradeRequest>()
            .add_systems(Update, show_incoming_request);
        app.world_mut().write_message(request());
        app.update();
        let dialogs = app.world().resource::<Messages<ShowSystemDialog>>();
        let dialog = dialogs.iter_current_update_messages().next().unwrap();
        assert_eq!(
            (
                dialog.kind,
                dialog.kicker.as_str(),
                dialog.title.as_str(),
                dialog.message.as_str()
            ),
            (
                SystemDialogKind::TradeRequest,
                "Trade",
                "Trade Request",
                "Alice wants to trade with you."
            )
        );
        assert_eq!(
            (
                dialog.button_label.as_str(),
                dialog.secondary_label.as_str()
            ),
            ("Accept", "Decline")
        );
        assert_eq!(
            dialog.correlation,
            app.world().resource::<PendingTradeRequest>().correlation
        );
        let mut app = App::new();
        app.add_message::<TradeRequestNotified>()
            .add_message::<ShowSystemDialog>()
            .init_resource::<PendingTradeRequest>()
            .add_systems(Update, show_incoming_request);
        app.world_mut().spawn(SystemDialogRoot::default());
        app.world_mut().write_message(request());
        app.update();
        assert!(!app.world().resource::<PendingTradeRequest>().is_pending());
        assert!(
            app.world()
                .resource::<Messages<ShowSystemDialog>>()
                .is_empty()
        );
    }

    #[test]
    fn choices_require_matching_token_and_forward_accept_or_decline() {
        for accept in [true, false] {
            let mut app = App::new();
            app.add_message::<SystemDialogChoice>()
                .add_message::<RespondTrade>()
                .init_resource::<PendingTradeRequest>()
                .add_systems(Update, claim_request_choice);
            app.world_mut()
                .resource_mut::<PendingTradeRequest>()
                .set(&request());
            let token = app.world().resource::<PendingTradeRequest>().correlation;
            app.world_mut().write_message(SystemDialogChoice {
                primary: true,
                kind: SystemDialogKind::TradeRequest,
                correlation: Some(999),
            });
            app.update();
            assert!(app.world().resource::<Messages<RespondTrade>>().is_empty());
            assert!(app.world().resource::<PendingTradeRequest>().is_pending());
            app.world_mut().write_message(SystemDialogChoice {
                primary: accept,
                kind: SystemDialogKind::TradeRequest,
                correlation: token,
            });
            app.update();
            let responses: Vec<_> = app
                .world()
                .resource::<Messages<RespondTrade>>()
                .iter_current_update_messages()
                .map(|m| m.accept)
                .collect();
            assert_eq!(responses, vec![accept]);
            assert!(!app.world().resource::<PendingTradeRequest>().is_pending());
        }
    }

    #[test]
    fn expiry_and_opened_only_despawn_matching_dialog() {
        for opened in [false, true] {
            let mut app = App::new();
            app.init_resource::<Time>()
                .init_resource::<PendingTradeRequest>()
                .add_message::<TradeOpened>()
                .add_systems(
                    Update,
                    (expire_pending_request, clear_on_trade_opened).chain(),
                );
            app.world_mut()
                .resource_mut::<PendingTradeRequest>()
                .set(&request());
            let token = app.world().resource::<PendingTradeRequest>().correlation;
            let owned = app
                .world_mut()
                .spawn(SystemDialogRoot::new(
                    None,
                    SystemDialogKind::TradeRequest,
                    token,
                ))
                .id();
            let other = app.world_mut().spawn(SystemDialogRoot::default()).id();
            if opened {
                app.world_mut().write_message(TradeOpened {
                    partner_char_id: 7,
                    partner_name: "Alice".into(),
                });
            } else {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(Duration::from_secs_f32(REQUEST_TTL_SECS + 1.0));
            }
            app.update();
            assert!(!app.world().resource::<PendingTradeRequest>().is_pending());
            assert!(app.world().get_entity(owned).is_err());
            assert!(app.world().get_entity(other).is_ok());
        }
    }
}
