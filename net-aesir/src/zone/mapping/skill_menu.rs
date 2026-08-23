use crate::proto::aesir::net;
use net_contract::events::{SkillMenuKind, SkillMenuOffered};

/// An unknown `kind` maps to `Items`, the kind whose ids are self-describing.
/// The offer is still rendered rather than dropped: the server is holding a
/// pending menu either way and the player must be able to answer or cancel it.
pub fn skill_menu(m: net::SkillMenu) -> SkillMenuOffered {
    let kind = match net::skill_menu::Kind::try_from(m.kind) {
        Ok(net::skill_menu::Kind::Skills) => SkillMenuKind::Skills,
        Ok(net::skill_menu::Kind::InventorySlots) => SkillMenuKind::InventorySlots,
        _ => SkillMenuKind::Items,
    };
    SkillMenuOffered {
        src_skill_id: m.src_skill_id,
        kind,
        entry_ids: m.entry_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(kind: i32) -> SkillMenuOffered {
        skill_menu(net::SkillMenu {
            src_skill_id: 98,
            kind,
            entry_ids: vec![1201, 1202],
        })
    }

    #[test]
    fn maps_every_kind() {
        assert_eq!(
            offer(net::skill_menu::Kind::Skills as i32).kind,
            SkillMenuKind::Skills
        );
        assert_eq!(
            offer(net::skill_menu::Kind::Items as i32).kind,
            SkillMenuKind::Items
        );
        assert_eq!(
            offer(net::skill_menu::Kind::InventorySlots as i32).kind,
            SkillMenuKind::InventorySlots
        );
    }

    #[test]
    fn carries_skill_and_entries() {
        let mapped = offer(net::skill_menu::Kind::Items as i32);
        assert_eq!(mapped.src_skill_id, 98);
        assert_eq!(mapped.entry_ids, vec![1201, 1202]);
    }

    #[test]
    fn unknown_kind_falls_back_to_items() {
        assert_eq!(offer(42).kind, SkillMenuKind::Items);
    }
}
