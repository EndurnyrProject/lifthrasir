use crate::infrastructure::assets::BgmNameTableAsset;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
pub use lifthrasir_runtime::audio::resources::*;

/// Resource that holds the BGM name table asset handle
/// This table maps map names to BGM file paths from mp3nametable.txt
#[derive(Resource, Debug, Default)]
#[auto_init_resource(plugin = crate::domain::audio::plugin::AudioPlugin)]
pub struct BgmNameTable {
    pub table_handle: Option<Handle<BgmNameTableAsset>>,
}
