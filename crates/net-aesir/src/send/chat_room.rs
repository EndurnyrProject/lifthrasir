use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::ChatRoomCommand;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net;
use crate::zone::{QuicZoneState, ZonePhase};

fn chat_room_body(command: &ChatRoomCommand) -> Body {
    match command.clone() {
        ChatRoomCommand::Create(s) => {
            Body::WaitingRoomCreateRequest(net::WaitingRoomCreateRequest {
                title: s.title,
                password: s.password,
                limit: s.seats,
                public: s.public,
            })
        }
        ChatRoomCommand::ChangeSettings(s) => {
            Body::WaitingRoomChangeStatusRequest(net::WaitingRoomChangeStatusRequest {
                title: s.title,
                password: s.password,
                limit: s.seats,
                public: s.public,
            })
        }
        ChatRoomCommand::Join { room_id, password } => {
            Body::WaitingRoomJoinRequest(net::WaitingRoomJoinRequest { room_id, password })
        }
        ChatRoomCommand::Leave => Body::WaitingRoomLeaveRequest(net::WaitingRoomLeaveRequest {}),
        ChatRoomCommand::Say(message) => {
            Body::WaitingRoomChatRequest(net::WaitingRoomChatRequest { message })
        }
        ChatRoomCommand::Kick(name) => {
            Body::WaitingRoomKickRequest(net::WaitingRoomKickRequest { name })
        }
        ChatRoomCommand::MakeOwner(name) => {
            Body::WaitingRoomChangeOwnerRequest(net::WaitingRoomChangeOwnerRequest { name })
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_chat_room_commands(
    mut commands: MessageReader<ChatRoomCommand>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, chat_room_body(command)) {
            error!("failed to send {command:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::ChatRoomSettings;

    fn settings() -> ChatRoomSettings {
        ChatRoomSettings {
            title: "Glast Heim run".into(),
            password: "gh22".into(),
            seats: 6,
            public: false,
        }
    }

    #[test]
    fn create_and_edit_carry_the_same_settings() {
        let Body::WaitingRoomCreateRequest(create) =
            chat_room_body(&ChatRoomCommand::Create(settings()))
        else {
            panic!("expected a create request");
        };
        assert_eq!(
            (create.title.as_str(), create.password.as_str()),
            ("Glast Heim run", "gh22")
        );
        assert_eq!((create.limit, create.public), (6, false));

        let Body::WaitingRoomChangeStatusRequest(edit) =
            chat_room_body(&ChatRoomCommand::ChangeSettings(settings()))
        else {
            panic!("expected a change status request");
        };
        assert_eq!((edit.title.as_str(), edit.limit), ("Glast Heim run", 6));
        assert_eq!((edit.password.as_str(), edit.public), ("gh22", false));
    }

    #[test]
    fn each_command_becomes_its_own_request() {
        let join = chat_room_body(&ChatRoomCommand::Join {
            room_id: 9,
            password: "wolf".into(),
        });
        assert!(matches!(
            join,
            Body::WaitingRoomJoinRequest(net::WaitingRoomJoinRequest { room_id: 9, ref password })
                if password == "wolf"
        ));
        assert!(matches!(
            chat_room_body(&ChatRoomCommand::Leave),
            Body::WaitingRoomLeaveRequest(_)
        ));
        assert!(matches!(
            chat_room_body(&ChatRoomCommand::Say("hi".into())),
            Body::WaitingRoomChatRequest(net::WaitingRoomChatRequest { ref message }) if message == "hi"
        ));
        assert!(matches!(
            chat_room_body(&ChatRoomCommand::Kick("Orm".into())),
            Body::WaitingRoomKickRequest(net::WaitingRoomKickRequest { ref name }) if name == "Orm"
        ));
        assert!(matches!(
            chat_room_body(&ChatRoomCommand::MakeOwner("Orm".into())),
            Body::WaitingRoomChangeOwnerRequest(net::WaitingRoomChangeOwnerRequest { ref name })
                if name == "Orm"
        ));
    }
}
