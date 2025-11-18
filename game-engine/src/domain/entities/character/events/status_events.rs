use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_event;

use crate::domain::entities::character::components::status::StatusParameter;

#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct StatusParameterChanged {
    pub entity: Entity,
    pub parameter: StatusParameter,
    pub new_value: u32,
    pub old_value: Option<u32>,
}

#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct StatusParametersBatchChanged {
    pub entity: Entity,
    pub changes: Vec<(StatusParameter, u32)>,
}
