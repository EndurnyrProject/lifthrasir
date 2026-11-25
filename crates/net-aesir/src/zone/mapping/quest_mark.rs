use net_contract::events::QuestMarkChanged;

use crate::proto::aesir::net;

pub fn quest_mark(m: net::QuestInfoIcon) -> QuestMarkChanged {
    if m.icon == 9999 {
        QuestMarkChanged::Clear { npc_id: m.npc_id }
    } else {
        QuestMarkChanged::Show {
            npc_id: m.npc_id,
            x: m.x as u16,
            y: m.y as u16,
            icon: m.icon,
            color: m.color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qtype_none_clears() {
        for color in [0, 1, 4, u32::MAX] {
            assert_eq!(
                quest_mark(net::QuestInfoIcon {
                    npc_id: 7,
                    x: 120,
                    y: 85,
                    icon: 9999,
                    color,
                }),
                QuestMarkChanged::Clear { npc_id: 7 }
            );
        }
    }

    #[test]
    fn unknown_values_are_kept() {
        assert_eq!(
            quest_mark(net::QuestInfoIcon {
                npc_id: 7,
                x: 120,
                y: 85,
                icon: 11,
                color: 4,
            }),
            QuestMarkChanged::Show {
                npc_id: 7,
                x: 120,
                y: 85,
                icon: 11,
                color: 4,
            }
        );
    }

    #[test]
    fn show_keeps_every_field() {
        assert_eq!(
            quest_mark(net::QuestInfoIcon {
                npc_id: 7,
                x: 120,
                y: 85,
                icon: 0,
                color: 1,
            }),
            QuestMarkChanged::Show {
                npc_id: 7,
                x: 120,
                y: 85,
                icon: 0,
                color: 1,
            }
        );
    }
}
