use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::domain::entities::character::components::status::StatusParameter;

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct StatusParameterChanged {
    pub entity: Entity,
    pub parameter: StatusParameter,
    pub new_value: u32,
    pub old_value: Option<u32>,
}

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct StatIncreaseRequested {
    pub status_id: u16,
    pub amount: u8,
}

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct SkillLearnRequested {
    pub skill_id: u32,
}
