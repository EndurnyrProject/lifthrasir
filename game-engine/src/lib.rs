// Public exports for the game engine
pub mod app;
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod plugins;
pub mod presentation;
pub mod utils;

// Re-export commonly used types
pub use app::{AuthenticationPlugin, LifthrasirPlugin, MapPlugin, NativeInputPlugin};
pub use domain::camera::CameraPlugin;
pub use domain::cart::CartPlugin;
pub use domain::character::{AssetCatalogPlugin, CharacterDomainPlugin};
pub use domain::combat::CombatPlugin;
pub use domain::entities::character::UnifiedCharacterEntityPlugin;
pub use domain::entities::hover_plugin::EntityHoverPlugin;
pub use domain::entities::movement::MovementPlugin;
pub use domain::entities::spawning::EntitySpawningPlugin;
pub use domain::equipment::EquipmentPlugin;
pub use domain::inventory::InventoryPlugin;
pub use domain::item_drop::ItemDropPlugin;
pub use domain::party::PartyPlugin;
pub use domain::settings::SettingsPlugin;
pub use infrastructure::accessory::{AccessoryDb, AccessoryDbPlugin};
pub use infrastructure::diagnostics::RoDiagnosticsPlugin;
pub use infrastructure::effect::EffectsPlugin;
pub use infrastructure::item::{ItemDb, ItemDbPlugin};
pub use infrastructure::job::JobSystemPlugin;
pub use infrastructure::skill::SkillSystemPlugin;
pub use infrastructure::status::StatusIconPlugin;
pub use infrastructure::weapon::{WeaponDb, WeaponDbPlugin};
pub use plugins::{AssetsPlugin, AudioPlugin, InputPlugin, WorldPlugin};
pub use presentation::rendering::VfxPlugin;
pub use presentation::ui::fps_counter::FpsCounterPlugin;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

pub struct CoreGamePlugins;

impl PluginGroup for CoreGamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(net_contract::NetContractPlugin)
            .add(RoDiagnosticsPlugin)
            .add(LifthrasirPlugin)
            .add(SettingsPlugin)
            .add(CameraPlugin)
            .add(AssetsPlugin)
            .add(JobSystemPlugin)
            .add(SkillSystemPlugin)
            .add(EffectsPlugin)
            .add(StatusIconPlugin)
            .add(VfxPlugin)
            .add(ItemDbPlugin)
            .add(AccessoryDbPlugin)
            .add(WeaponDbPlugin)
            .add(AudioPlugin)
            .add(AssetCatalogPlugin)
            .add(EntitySpawningPlugin)
            .add(CharacterDomainPlugin)
            .add(AuthenticationPlugin)
            .add(WorldPlugin)
            .add(MovementPlugin)
            .add(EntityHoverPlugin)
            .add(CombatPlugin)
            .add(InventoryPlugin)
            .add(CartPlugin)
            .add(PartyPlugin)
            .add(ItemDropPlugin)
            .add(EquipmentPlugin)
            .add(InputPlugin)
            .add(NativeInputPlugin)
            .add(FpsCounterPlugin)
    }
}
