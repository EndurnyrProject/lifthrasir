use super::{resource::Storage, systems};
use crate::core::state::GameState;
use bevy::prelude::*;

pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Storage>()
            .add_systems(
                Update,
                (
                    systems::apply_storage_opened,
                    systems::apply_storage_item_deltas.after(systems::apply_storage_opened),
                    systems::apply_storage_close.after(systems::apply_storage_item_deltas),
                ),
            )
            .add_systems(OnExit(GameState::InGame), systems::reset_storage);
    }
}
