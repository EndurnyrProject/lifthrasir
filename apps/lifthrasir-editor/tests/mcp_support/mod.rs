//! Drives the MCP dispatch headless: an editor app with the dispatch plugin
//! and a channel standing in for the server.

use bevy::prelude::*;
use lifthrasir_editor::mcp::{
    self,
    call::{AgentCall, AgentResult, Call},
    dispatch::AgentInbox,
};
use std::path::Path;
use tokio::sync::{
    mpsc::{UnboundedSender, unbounded_channel},
    oneshot,
};

/// `common::editor_app` plus MCP dispatch; send calls on the returned sender.
pub fn mcp_app(data_folder: &Path) -> (App, UnboundedSender<AgentCall>) {
    let mut app = crate::common::editor_app(data_folder);
    let (tx, rx) = unbounded_channel();
    app.insert_resource(AgentInbox(rx));
    mcp::dispatch_plugin(&mut app);
    (app, tx)
}

/// Queues `call`; the result arrives on the returned receiver once dispatched.
pub fn send(tx: &UnboundedSender<AgentCall>, call: Call) -> oneshot::Receiver<AgentResult> {
    let (reply, result) = oneshot::channel();
    tx.send(AgentCall { call, reply }).unwrap();
    result
}

/// Sends `call`, runs one update and returns its result.
pub fn call(app: &mut App, tx: &UnboundedSender<AgentCall>, call: Call) -> AgentResult {
    let mut result = send(tx, call);
    app.update();
    result.try_recv().expect("no reply after one update")
}
