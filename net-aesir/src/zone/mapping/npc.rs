use crate::proto::aesir::net;
use crate::proto::aesir::net::npc_dialog::Expect;
use bevy::prelude::warn;
use net_contract::dto::NpcDialogExpect;
use net_contract::events::NpcDialogReceived;

pub fn npc_dialog(d: net::NpcDialog) -> NpcDialogReceived {
    let expect = match Expect::try_from(d.expect) {
        Ok(Expect::Next) => NpcDialogExpect::Next,
        Ok(Expect::Menu) => NpcDialogExpect::Menu,
        Ok(Expect::InputInt) => NpcDialogExpect::InputInt,
        Ok(Expect::InputStr) => NpcDialogExpect::InputStr,
        Ok(Expect::Close) => NpcDialogExpect::Close,
        Err(_) => {
            warn!(
                "unknown NpcDialog.expect {} for npc {}; closing dialog",
                d.expect, d.npc_id
            );
            NpcDialogExpect::Close
        }
    };

    NpcDialogReceived {
        npc_id: d.npc_id,
        text: d.text,
        expect,
        options: d.options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(expect: Expect, options: Vec<&str>) -> net::NpcDialog {
        net::NpcDialog {
            npc_id: 150001,
            text: "hello".into(),
            expect: expect as i32,
            options: options.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn maps_next() {
        let d = npc_dialog(dialog(Expect::Next, vec![]));
        assert_eq!(d.expect, NpcDialogExpect::Next);
        assert_eq!(d.npc_id, 150001);
        assert_eq!(d.text, "hello");
    }

    #[test]
    fn maps_menu_with_options() {
        let d = npc_dialog(dialog(Expect::Menu, vec!["Yes", "No"]));
        assert_eq!(d.expect, NpcDialogExpect::Menu);
        assert_eq!(d.options, vec!["Yes".to_string(), "No".to_string()]);
    }

    #[test]
    fn maps_input_int() {
        let d = npc_dialog(dialog(Expect::InputInt, vec![]));
        assert_eq!(d.expect, NpcDialogExpect::InputInt);
    }

    #[test]
    fn maps_input_str() {
        let d = npc_dialog(dialog(Expect::InputStr, vec![]));
        assert_eq!(d.expect, NpcDialogExpect::InputStr);
    }

    #[test]
    fn maps_close() {
        let d = npc_dialog(dialog(Expect::Close, vec![]));
        assert_eq!(d.expect, NpcDialogExpect::Close);
    }
}
