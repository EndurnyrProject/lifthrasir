use super::accessory::AccessoryDbPlugin;
use super::effect::{process_loaded_effect_data, start_loading_effect_data};
use super::item::ItemDbPlugin;
use super::job::JobSystemPlugin;
use super::skill::SkillSystemPlugin;
use super::status::StatusIconPlugin;
use super::weapon::WeaponDbPlugin;
use super::{
    AccessoryDataAsset, EffectDataAsset, ItemDataAsset, JobDataAsset, SkillDataAsset,
    StatusIconDataAsset, WeaponDataAsset,
};
use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

/// Select startup catalog loading separately from raw content loaders.
pub struct ContentCatalogPlugin;

impl Plugin for ContentCatalogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            RonAssetPlugin::<JobDataAsset>::new(&["ron"]),
            RonAssetPlugin::<ItemDataAsset>::new(&["ron"]),
            RonAssetPlugin::<SkillDataAsset>::new(&["ron"]),
            RonAssetPlugin::<EffectDataAsset>::new(&["ron"]),
            RonAssetPlugin::<AccessoryDataAsset>::new(&["ron"]),
            RonAssetPlugin::<WeaponDataAsset>::new(&["ron"]),
            RonAssetPlugin::<StatusIconDataAsset>::new(&["ron"]),
            JobSystemPlugin,
            SkillSystemPlugin,
            ItemDbPlugin,
            AccessoryDbPlugin,
            WeaponDbPlugin,
            StatusIconPlugin,
        ))
        .add_systems(Startup, start_loading_effect_data)
        .add_systems(Update, process_loaded_effect_data);
    }
}
