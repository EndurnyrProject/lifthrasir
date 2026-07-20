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
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
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
}
