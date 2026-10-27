use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::{GameplayNoticeReceived, InventoryAddRejected, ItemSkillOffered};

use super::super::mapping::gameplay::{gameplay_notice, inventory_add_rejected, item_skill_offer};
use crate::{dispatch::IncomingMessage, envelope::Body};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_gameplay(
    mut incoming: MessageReader<IncomingMessage>,
    mut offers: MessageWriter<ItemSkillOffered>,
    mut rejections: MessageWriter<InventoryAddRejected>,
    mut notices: MessageWriter<GameplayNoticeReceived>,
) {
    for message in incoming.read() {
        match &message.body {
            Body::ItemSkillReady(offer) => {
                if let Some(offer) = item_skill_offer(*offer) {
                    offers.write(offer);
                }
            }
            Body::ItemAddFailed(failure) => {
                if let Some(failure) = inventory_add_rejected(*failure) {
                    rejections.write(failure);
                }
            }
            Body::SystemNotice(notice) => {
                if let Some(notice) = gameplay_notice(*notice) {
                    notices.write(notice);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{channels::GAMEPLAY, envelope::Body, proto::aesir::net};
    use net_contract::dto::ItemSkillTargetKind;

    #[test]
    fn item_skill_offer_preserves_target_level_and_identity() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ItemSkillOffered>()
            .add_message::<InventoryAddRejected>()
            .add_message::<GameplayNoticeReceived>()
            .add_systems(Update, zone_drain_gameplay);
        app.world_mut().write_message(IncomingMessage {
            channel: GAMEPLAY,
            body: Body::ItemSkillReady(net::ItemSkillReady {
                skill_id: 28,
                level: 7,
                target_kind: net::item_skill_ready::TargetKind::Unit as i32,
            }),
        });
        app.update();
        let offers: Vec<_> = app
            .world()
            .resource::<Messages<ItemSkillOffered>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(offers.len(), 1);
        assert_eq!((offers[0].skill_id, offers[0].level), (28, 7));
        assert_eq!(offers[0].target_kind, ItemSkillTargetKind::Unit);
    }

    #[test]
    fn feedback_preserves_known_and_unknown_reasons_but_not_success() {
        use net_contract::dto::{GameplayNoticeKind as Notice, ItemAddFailure as Failure};
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ItemSkillOffered>()
            .add_message::<InventoryAddRejected>()
            .add_message::<GameplayNoticeReceived>()
            .add_systems(Update, zone_drain_gameplay);
        for result in [0, 1, 2, 3, 4, 5, 6, 7, -17] {
            app.world_mut().write_message(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::ItemAddFailed(net::ItemAddFailed { result }),
            });
        }
        for id in [0, 1, 2, 3, 4, 5, 6, -23] {
            app.world_mut().write_message(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::SystemNotice(net::SystemNotice { id }),
            });
        }
        app.update();
        let failures: Vec<_> = app
            .world()
            .resource::<Messages<InventoryAddRejected>>()
            .iter_current_update_messages()
            .map(|event| event.reason)
            .collect();
        assert_eq!(
            failures,
            [
                Failure::Invalid,
                Failure::Overweight,
                Failure::Item,
                Failure::TooManyItems,
                Failure::TooManyOfItem,
                Failure::RefusedTime,
                Failure::StackLimit,
                Failure::Unknown(-17)
            ]
        );
        let notices: Vec<_> = app
            .world()
            .resource::<Messages<GameplayNoticeReceived>>()
            .iter_current_update_messages()
            .map(|event| event.kind)
            .collect();
        assert_eq!(
            notices,
            [
                Notice::Busy,
                Notice::ImpossibleSkillArea,
                Notice::CannotEquipItem,
                Notice::MapCannotTeleport,
                Notice::MapCannotMemo,
                Notice::MapCannotUseSkill,
                Notice::Unknown(-23)
            ]
        );
    }
}
