use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::skill_menu::skill_menu;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::SkillMenuOffered;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_skill_menu(
    mut incoming: MessageReader<IncomingMessage>,
    mut offers: MessageWriter<SkillMenuOffered>,
) {
    for msg in incoming.read() {
        if let Body::SkillMenu(m) = msg.body.clone() {
            offers.write(skill_menu(m));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::WORLD;
    use crate::proto::aesir::net;
    use net_contract::events::SkillMenuKind;

    fn drained(body: Body) -> Vec<SkillMenuOffered> {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<SkillMenuOffered>()
            .add_systems(Update, zone_drain_skill_menu);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body,
            });
        app.update();

        app.world()
            .resource::<Messages<SkillMenuOffered>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    #[test]
    fn skill_menu_produces_one_offer() {
        let offers = drained(Body::SkillMenu(net::SkillMenu {
            src_skill_id: 98,
            kind: net::skill_menu::Kind::Items as i32,
            entry_ids: vec![1201],
        }));

        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].src_skill_id, 98);
        assert_eq!(offers[0].kind, SkillMenuKind::Items);
        assert_eq!(offers[0].entry_ids, vec![1201]);
    }

    #[test]
    fn unrelated_body_produces_nothing() {
        assert!(drained(Body::MountResult(net::MountResult { result: 0 })).is_empty());
    }
}
