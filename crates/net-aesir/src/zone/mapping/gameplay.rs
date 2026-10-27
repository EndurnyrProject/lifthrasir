//! Wire gameplay offers and feedback mapped without inventing defaults.

use bevy::prelude::warn;
use net_contract::{
    dto::{GameplayNoticeKind, ItemAddFailure, ItemSkillTargetKind},
    events::{GameplayNoticeReceived, InventoryAddRejected, ItemSkillOffered},
};

use crate::proto::aesir::net;

pub fn item_skill_offer(offer: net::ItemSkillReady) -> Option<ItemSkillOffered> {
    if offer.skill_id == 0 || offer.level == 0 {
        warn!("invalid item skill offer: {offer:?}");
        return None;
    }
    let target_kind = match net::item_skill_ready::TargetKind::try_from(offer.target_kind) {
        Ok(net::item_skill_ready::TargetKind::Self_) => ItemSkillTargetKind::SelfTarget,
        Ok(net::item_skill_ready::TargetKind::Unit) => ItemSkillTargetKind::Unit,
        Ok(net::item_skill_ready::TargetKind::Ground) => ItemSkillTargetKind::Ground,
        Err(_) => {
            warn!("unknown item skill target kind {}", offer.target_kind);
            return None;
        }
    };
    Some(ItemSkillOffered {
        skill_id: offer.skill_id,
        level: offer.level,
        target_kind,
    })
}

pub fn inventory_add_rejected(failure: net::ItemAddFailed) -> Option<InventoryAddRejected> {
    use net::ItemAddResult as Wire;
    let reason = match Wire::try_from(failure.result) {
        Ok(Wire::Success) => {
            warn!("ItemAddFailed carried success");
            return None;
        }
        Ok(Wire::Invalid) => ItemAddFailure::Invalid,
        Ok(Wire::Overweight) => ItemAddFailure::Overweight,
        Ok(Wire::Item) => ItemAddFailure::Item,
        Ok(Wire::Overitem) => ItemAddFailure::TooManyItems,
        Ok(Wire::Overamount) => ItemAddFailure::TooManyOfItem,
        Ok(Wire::RefusedTime) => ItemAddFailure::RefusedTime,
        Ok(Wire::Stacklimit) => ItemAddFailure::StackLimit,
        Err(_) => ItemAddFailure::Unknown(failure.result),
    };
    Some(InventoryAddRejected { reason })
}

pub fn gameplay_notice(notice: net::SystemNotice) -> Option<GameplayNoticeReceived> {
    use net::SystemNoticeId as Wire;
    let kind = match Wire::try_from(notice.id) {
        Ok(Wire::Unspecified) => {
            warn!("SystemNotice carried unspecified id");
            return None;
        }
        Ok(Wire::Busy) => GameplayNoticeKind::Busy,
        Ok(Wire::ImpossibleSkillArea) => GameplayNoticeKind::ImpossibleSkillArea,
        Ok(Wire::CanNotEquipItem) => GameplayNoticeKind::CannotEquipItem,
        Ok(Wire::MapCantTeleport) => GameplayNoticeKind::MapCannotTeleport,
        Ok(Wire::MapCantMemo) => GameplayNoticeKind::MapCannotMemo,
        Ok(Wire::MapCantUseSkill) => GameplayNoticeKind::MapCannotUseSkill,
        Err(_) => GameplayNoticeKind::Unknown(notice.id),
    };
    Some(GameplayNoticeReceived { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_offers_never_become_self_casts() {
        for (skill_id, level, target_kind) in [(0, 1, 0), (28, 0, 0), (28, 1, 99)] {
            assert!(
                item_skill_offer(net::ItemSkillReady {
                    skill_id,
                    level,
                    target_kind
                })
                .is_none()
            );
        }
        for (target_kind, expected) in [
            (0, ItemSkillTargetKind::SelfTarget),
            (1, ItemSkillTargetKind::Unit),
            (2, ItemSkillTargetKind::Ground),
        ] {
            assert_eq!(
                item_skill_offer(net::ItemSkillReady {
                    skill_id: 28,
                    level: 1,
                    target_kind
                })
                .unwrap()
                .target_kind,
                expected
            );
        }
    }
}
