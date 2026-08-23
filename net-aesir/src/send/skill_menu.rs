use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::AnswerSkillMenu;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::SkillMenuReply;
use crate::zone::{QuicZoneState, ZonePhase};

/// The server takes at most three catalysts, so the reply is truncated here
/// rather than trusting the UI to cap it.
const MAX_EXTRAS: usize = 3;

fn skill_menu_reply_body(c: &AnswerSkillMenu) -> Body {
    Body::SkillMenuReply(SkillMenuReply {
        src_skill_id: c.src_skill_id,
        selected_id: c.selected_id,
        extra_ids: c.extra_ids.iter().copied().take(MAX_EXTRAS).collect(),
        cancel: c.cancel,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_skill_menu_answers(
    mut events: MessageReader<AnswerSkillMenu>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, skill_menu_reply_body(ev)) {
            error!("failed to send SkillMenuReply: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_of(answer: AnswerSkillMenu) -> SkillMenuReply {
        match skill_menu_reply_body(&answer) {
            Body::SkillMenuReply(r) => r,
            other => panic!("expected Body::SkillMenuReply, got {other:?}"),
        }
    }

    fn reply(extras: Vec<u32>) -> SkillMenuReply {
        body_of(AnswerSkillMenu {
            src_skill_id: 98,
            selected_id: 1201,
            extra_ids: extras,
            cancel: false,
        })
    }

    #[test]
    fn carries_skill_selection_and_catalysts() {
        let r = reply(vec![1000, 994]);
        assert_eq!(r.src_skill_id, 98);
        assert_eq!(r.selected_id, 1201);
        assert_eq!(r.extra_ids, vec![1000, 994]);
        assert!(!r.cancel);
    }

    #[test]
    fn truncates_extras_to_three() {
        assert_eq!(
            reply(vec![1000, 1000, 1000, 994]).extra_ids,
            vec![1000, 1000, 1000]
        );
    }

    // Cancelling travels in its own field: the server judges `selected_id` only
    // against the offered ids, and inventory-slot menus really do offer slot 0.
    #[test]
    fn carries_the_cancel_flag() {
        let r = body_of(AnswerSkillMenu {
            src_skill_id: 40,
            selected_id: 0,
            extra_ids: Vec::new(),
            cancel: true,
        });
        assert!(r.cancel);
        assert_eq!(r.src_skill_id, 40);
    }

    #[test]
    fn selecting_inventory_slot_zero_is_not_a_cancel() {
        let r = body_of(AnswerSkillMenu {
            src_skill_id: 40,
            selected_id: 0,
            extra_ids: Vec::new(),
            cancel: false,
        });
        assert!(!r.cancel);
        assert_eq!(r.selected_id, 0);
    }
}
