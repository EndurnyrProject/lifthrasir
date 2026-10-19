use crate::core::state::GameState;
use crate::domain::system_sets::CharacterFlowSystems;
use crate::presentation::ui::events::{DialogSeverity, ShowSystemDialog, SystemDialogKind};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::ZoneDisconnected;

fn disconnect_message(reason: &str) -> String {
    format!(
        "You have been disconnected from the realm. Please check your connection and try again.\n\n{reason}"
    )
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ZoneEntry)
)]
pub fn handle_zone_disconnected(
    mut events: MessageReader<ZoneDisconnected>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    for event in events.read() {
        warn!("Zone disconnected: {}", event.reason);
        dialogs.write(ShowSystemDialog {
            severity: DialogSeverity::Error,
            kind: SystemDialogKind::Generic,
            kicker: "Connection".into(),
            title: "Disconnected".into(),
            message: disconnect_message(&event.reason),
            code: String::new(),
            button_label: "OK".into(),
            secondary_label: String::new(),
            confirm_state: Some(GameState::Login),
            correlation: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnect_message_includes_reason() {
        let text = disconnect_message("connection lost");
        assert!(text.contains("disconnected from the realm"));
        assert!(text.ends_with("connection lost"));
    }

    #[test]
    fn zone_disconnect_writes_login_dialog() {
        let mut app = App::new();
        app.add_message::<ZoneDisconnected>()
            .add_message::<ShowSystemDialog>()
            .add_systems(Update, handle_zone_disconnected);
        app.world_mut().write_message(ZoneDisconnected {
            reason: "zone server did not answer the liveness probe".into(),
        });

        app.update();

        let messages = app.world().resource::<Messages<ShowSystemDialog>>();
        let mut cursor = messages.get_cursor();
        let dialogs = cursor.read(messages).collect::<Vec<_>>();
        assert_eq!(dialogs.len(), 1);
        assert_eq!(dialogs[0].title, "Disconnected");
        assert!(dialogs[0].message.ends_with("liveness probe"));
        assert_eq!(dialogs[0].confirm_state, Some(GameState::Login));
    }
}
