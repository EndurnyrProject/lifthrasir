use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomunculusState {
    pub world_id: u32,
    pub name: String,
    pub level: u32,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub exp: u64,
    pub next_exp: u64,
    pub hunger: u32,
    pub intimacy_hundredths: u32,
    pub skill_points: u32,
}

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct HomunculusStateReceived(pub HomunculusState);

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct HomunculusResultReceived {
    pub request_id: u64,
    pub success: bool,
    pub error: i32,
    pub state: Option<HomunculusState>,
}
