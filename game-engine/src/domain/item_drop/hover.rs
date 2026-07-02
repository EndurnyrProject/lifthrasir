use bevy::prelude::*;

/// The floor item currently under the cursor, if any. Set by the sprite picking
/// observers (`entities::picking`); floor items are not `NetworkEntity`, so they
/// cannot ride `entities::hover::CurrentlyHoveredEntity`.
#[derive(Resource, Default)]
pub struct HoveredFloorItem(pub Option<Entity>);
