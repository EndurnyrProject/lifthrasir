//! Agent calls enter the Bevy world here, one at a time, never while a human
//! gesture is in progress, and each leaves at most one undo entry.

use super::{
    call::{AgentCall, AgentError, AgentResult, Call},
    handlers,
};
use crate::{map::open::EditorOperation, shell::undo::UndoStack};
use bevy::prelude::*;
use tokio::sync::mpsc::UnboundedReceiver;

/// Where the MCP server's calls arrive.
#[derive(Resource)]
pub struct AgentInbox(pub UnboundedReceiver<AgentCall>);

/// Runs every queued call, unless a mouse button is held or was released this
/// frame (a stroke, gizmo or widget drag records its edit on release): the
/// calls then wait. Calls whose client gave up are dropped unrun, so a retry
/// cannot apply an edit twice.
pub fn drain_calls(world: &mut World) {
    let gesture = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|mouse| {
            mouse.get_pressed().next().is_some() || mouse.get_just_released().next().is_some()
        });
    if gesture {
        return;
    }
    while let Ok(AgentCall { call, reply }) = world.resource_mut::<AgentInbox>().0.try_recv() {
        if reply.is_closed() {
            continue;
        }
        let _ = reply.send(run(world, call));
    }
}

/// Runs one call and folds whatever it pushed into one `Agent: <tool>` entry.
/// Undo and redo move existing entries, so they keep their labels.
fn run(world: &mut World, call: Call) -> AgentResult {
    if let Some(busy) = busy(world)
        && !matches!(call, Call::Status)
    {
        return Err(AgentError::Busy(busy));
    }
    if matches!(call, Call::Undo | Call::Redo) {
        return handlers::run(world, call);
    }
    let mark = world.resource::<UndoStack>().depth();
    let label = format!("Agent: {}", call.name());
    let result = handlers::run(world, call);
    world.resource_mut::<UndoStack>().squash_since(mark, label);
    result
}

fn busy(world: &World) -> Option<String> {
    match world.resource::<EditorOperation>() {
        EditorOperation::Loading { name } => Some(format!("loading {name}")),
        EditorOperation::Saving { name } => Some(format!("saving {name}")),
        EditorOperation::Idle | EditorOperation::Saved { .. } => None,
    }
}
