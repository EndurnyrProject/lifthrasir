//! Map-wide lighting the editor applies itself, like every host does.

use bevy::{camera::Exposure, prelude::*};
use lifthrasir_data::lif::LifMap;
use lifthrasir_runtime::world::lighting::map_ambient_light;

/// Inserts the map's ambient light and sets `camera`'s exposure from `meta`.
pub fn apply_map_lighting(commands: &mut Commands, meta: &LifMap, camera: Entity) {
    commands.insert_resource(map_ambient_light(meta));
    commands.entity(camera).insert(Exposure {
        ev100: meta.exposure_ev100,
    });
}
