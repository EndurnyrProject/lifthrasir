use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;

/// RO's press-then-click targeting state machine: `resolve_skill_cast` arms it
/// for entity/ground skills, and the click handlers (a later slice) consume it.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub enum TargetingMode {
    #[default]
    Idle,
    AwaitingEntity {
        skill_id: u32,
        level: u32,
    },
    AwaitingGround {
        skill_id: u32,
        level: u32,
    },
}
