use super::catalog::{process_loaded_status_icons, start_loading_status_icons};
use bevy::prelude::*;

/// Loads `data/ron/status_icons.ron` into a `StatusIconCatalog` resource. The
/// `StatusIconDataAsset` RON loader is registered centrally in `AssetsPlugin`,
/// matching the effect catalog. Failure degrades gracefully (logged, no panic).
pub struct StatusIconPlugin;

impl Plugin for StatusIconPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_status_icons)
            .add_systems(Update, process_loaded_status_icons);
    }
}
