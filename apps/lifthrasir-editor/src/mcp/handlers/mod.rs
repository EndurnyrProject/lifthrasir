//! One handler per tool. Each validates everything before its first change.

pub mod session;

use super::call::{AgentResult, Call};
use crate::shell::node::NodeKind;
use bevy::prelude::*;

/// How tools name a node kind.
pub fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Prop => "prop",
        NodeKind::Sound => "sound",
        NodeKind::Effect => "effect",
        NodeKind::PointLight => "point_light",
        NodeKind::SpotLight => "spot_light",
        NodeKind::Sun => "sun",
    }
}

/// Runs `call` against the world.
pub fn run(world: &mut World, call: Call) -> AgentResult {
    match call {
        Call::Status => session::editor_status(world),
        Call::Undo => session::undo(world),
        Call::Redo => session::redo(world),
        Call::ListMaps => session::list(world),
        Call::OpenMap(p) => session::open(world, p),
        Call::NewMap(p) => session::new_map(world, p),
        Call::SaveMap => session::save_map(world),
    }
}
