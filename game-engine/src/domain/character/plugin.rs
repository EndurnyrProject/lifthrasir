use crate::app::character_domain_plugin::CharacterDomainAutoPlugin;
use crate::app::zone_domain_plugin::ZoneDomainAutoPlugin;
use crate::domain::entities::character::UnifiedCharacterEntityPlugin;
use crate::domain::entities::character::states::setup_character_state_machines;
use crate::domain::entities::sprite_rendering::plugin::GenericSpriteRenderingPlugin;
use bevy::prelude::*;

/// Character Domain Plugin
///
/// Composes character functionality with proper dependency order:
/// 1. Add sub-plugins in correct order:
///    - StateMachinePlugin (via setup_character_state_machines) - state transitions
///    - GenericSpriteRenderingPlugin - sprite hierarchy and rendering
///    - UnifiedCharacterEntityPlugin - character entity management (auto-plugin)
/// 2. Add CharacterDomainAutoPlugin (all domain logic via auto_plugin)
pub struct CharacterDomainPlugin;

impl Plugin for CharacterDomainPlugin {
    fn build(&self, app: &mut App) {
        // Add sub-plugins that UnifiedCharacterEntityPlugin depends on
        // (must be added before the auto-plugin)
        setup_character_state_machines(app);
        app.add_plugins(GenericSpriteRenderingPlugin);

        // Add unified character entity plugin (pure auto-plugin)
        app.add_plugins(UnifiedCharacterEntityPlugin);

        // Add domain auto-plugins (all domain events and systems)
        app.add_plugins(CharacterDomainAutoPlugin);
        app.add_plugins(ZoneDomainAutoPlugin);

        debug!("CharacterDomainPlugin initialized");
    }
}
