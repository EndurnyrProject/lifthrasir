use super::components::CharacterSelectionState;
use crate::app::character_domain_plugin::CharacterDomainAutoPlugin;
use crate::domain::entities::character::states::setup_character_state_machines;
use crate::domain::entities::character::UnifiedCharacterEntityPlugin;
use crate::domain::entities::sprite_rendering::plugin::GenericSpriteRenderingPlugin;
use crate::infrastructure::networking::protocol::character::{
    BlockedCharactersReceived, CharacterCreated, CharacterCreationFailed, CharacterDeleted,
    CharacterDeletionFailed, CharacterInfoPageReceived, CharacterServerConnected,
    CharacterSlotInfoReceived, PingReceived, SecondPasswordRequested, ZoneServerInfoReceived,
};
use crate::infrastructure::networking::protocol::zone::{
    AccountIdReceived, ChatReceived, EntityNameAllReceived, EntityNameReceived,
    ParameterChanged, ZoneEntryRefused, ZoneServerConnected as ZoneServerConnectedProtocol,
};
use bevy::prelude::*;

/// Character Domain Plugin
///
/// Composes character functionality with proper dependency order:
/// 1. Initialize resources (CharacterSelectionState)
/// 2. Add sub-plugins in correct order:
///    - StateMachinePlugin (via setup_character_state_machines) - state transitions
///    - GenericSpriteRenderingPlugin - sprite hierarchy and rendering
///    - UnifiedCharacterEntityPlugin - character entity management (auto-plugin)
/// 3. Register protocol layer events (networking infrastructure)
/// 4. Register infrastructure systems (char_server_update, zone_server_update, time_sync)
/// 5. Add CharacterDomainAutoPlugin (all domain logic via auto_plugin)
pub struct CharacterDomainPlugin;

impl Plugin for CharacterDomainPlugin {
    fn build(&self, app: &mut App) {
        // 1. Initialize character selection state resource
        app.insert_resource(CharacterSelectionState::default());

        // 2. Add sub-plugins that UnifiedCharacterEntityPlugin depends on
        // (must be added before the auto-plugin)
        setup_character_state_machines(app);
        app.add_plugins(GenericSpriteRenderingPlugin);

        // Add unified character entity plugin (pure auto-plugin)
        app.add_plugins(UnifiedCharacterEntityPlugin);

        // 3. Register protocol layer events (from networking infrastructure)
        app.add_message::<CharacterServerConnected>()
            .add_message::<CharacterCreated>()
            .add_message::<CharacterCreationFailed>()
            .add_message::<CharacterDeleted>()
            .add_message::<CharacterDeletionFailed>()
            .add_message::<ZoneServerInfoReceived>()
            .add_message::<PingReceived>()
            .add_message::<SecondPasswordRequested>()
            .add_message::<CharacterInfoPageReceived>()
            .add_message::<CharacterSlotInfoReceived>()
            .add_message::<BlockedCharactersReceived>();

        // Zone protocol events (new modular architecture)
        app.add_message::<ZoneServerConnectedProtocol>()
            .add_message::<AccountIdReceived>()
            .add_message::<ZoneEntryRefused>()
            .add_message::<EntityNameReceived>()
            .add_message::<EntityNameAllReceived>()
            .add_message::<ParameterChanged>()
            .add_message::<ChatReceived>();

        // 4. Register infrastructure systems (networking layer)
        app.add_systems(
            Update,
            (
                crate::infrastructure::networking::client::char_server_update_system,
                crate::infrastructure::networking::client::zone_server_update_system,
                crate::infrastructure::networking::client::time_sync_system,
            )
                .chain(),
        );

        // 5. Add domain auto-plugin (all domain events and systems)
        app.add_plugins(CharacterDomainAutoPlugin);

        info!("CharacterDomainPlugin initialized");
    }
}
