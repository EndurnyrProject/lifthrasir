// Public exports for the game engine
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod utils;

// Re-export commonly used types
pub use crate::core::state::LifthrasirPlugin;
pub use crate::domain::audio::AudioPlugin;
pub use crate::domain::authentication::AuthenticationPlugin;
pub use crate::domain::camera::CameraPlugin;
pub use crate::domain::cart::CartPlugin;
pub use crate::domain::character::CharacterDomainPlugin;
pub use crate::domain::combat::CombatPlugin;
pub use crate::domain::emote::EmotePlugin;
pub use crate::domain::entities::character::UnifiedCharacterEntityPlugin;
pub use crate::domain::entities::hover_plugin::EntityHoverPlugin;
pub use crate::domain::entities::movement::MovementPlugin;
pub use crate::domain::entities::spawning::EntitySpawningPlugin;
pub use crate::domain::equipment::EquipmentPlugin;
pub use crate::domain::guild::GuildPlugin;
pub use crate::domain::input::{InputPlugin, NativeInputPlugin};
pub use crate::domain::inventory::InventoryPlugin;
pub use crate::domain::item_drop::ItemDropPlugin;
pub use crate::domain::party::PartyPlugin;
pub use crate::domain::settings::SettingsRuntimePlugin;
pub use crate::domain::skill_units::SkillUnitsPlugin;
pub use crate::domain::storage::StoragePlugin;
pub use crate::domain::trade::TradePlugin;
pub use crate::domain::world::GltfMapPlugin;
pub use crate::domain::world::WorldDomainPlugin;
pub use crate::infrastructure::assets::ClientConfigPlugin;
pub use crate::infrastructure::diagnostics::RoDiagnosticsPlugin;
pub use crate::infrastructure::effect::EffectsPlugin;
pub use crate::presentation::rendering::{MapPlugin, VfxPlugin};

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

pub struct ClientPlugins;

impl PluginGroup for ClientPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(RoDiagnosticsPlugin)
            .add(LifthrasirPlugin)
            .add(SettingsRuntimePlugin)
            .add(CameraPlugin)
            .add(ClientConfigPlugin)
            .add(EffectsPlugin)
            .add(SkillUnitsPlugin)
            .add(VfxPlugin)
            .add(AudioPlugin)
            .add(EntitySpawningPlugin)
            .add(CharacterDomainPlugin)
            .add(AuthenticationPlugin)
            .add(WorldDomainPlugin)
            .add(MovementPlugin)
            .add(EntityHoverPlugin)
            .add(CombatPlugin)
            .add(InventoryPlugin)
            .add(CartPlugin)
            .add(StoragePlugin)
            .add(TradePlugin)
            .add(EmotePlugin)
            .add(PartyPlugin)
            .add(GuildPlugin)
            .add(ItemDropPlugin)
            .add(EquipmentPlugin)
            .add(InputPlugin)
            .add(NativeInputPlugin)
            .add(MapPlugin)
            .add(GltfMapPlugin)
    }
}
