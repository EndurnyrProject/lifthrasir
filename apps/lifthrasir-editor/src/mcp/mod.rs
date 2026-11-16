//! The editor's MCP surface: agents call tools that run inside the Bevy world
//! as one undoable step each. [`dispatch_plugin`] is the Bevy side; the
//! caller inserts the [`dispatch::AgentInbox`] its calls arrive on.

pub mod call;
pub mod coords;
pub mod dispatch;
pub mod handlers;

use bevy::prelude::*;

/// Runs agent calls from [`dispatch::AgentInbox`] every frame.
pub fn dispatch_plugin(app: &mut App) {
    app.add_systems(Update, dispatch::drain_calls);
}
