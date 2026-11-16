//! What an agent call carries into the Bevy world and what comes back. No MCP
//! protocol types live here, so handlers are testable without a server.

use serde_json::Value;
use thiserror::Error;
use tokio::sync::oneshot;

/// One tool call and where its result goes.
pub struct AgentCall {
    pub call: Call,
    pub reply: oneshot::Sender<AgentResult>,
}

/// Every tool an agent can call, with its parameters.
#[derive(Debug)]
pub enum Call {
    Status,
    Undo,
    Redo,
}

impl Call {
    /// The MCP tool name; also the undo label suffix (`Agent: <name>`).
    pub fn name(&self) -> &'static str {
        match self {
            Call::Status => "editor_status",
            Call::Undo => "undo",
            Call::Redo => "redo",
        }
    }
}

/// A tool's result: JSON for the agent, plus a PNG for captures.
#[derive(Debug, Default)]
pub struct AgentOutput {
    pub json: Value,
    pub png: Option<Vec<u8>>,
}

impl AgentOutput {
    pub fn json(json: Value) -> Self {
        Self { json, png: None }
    }
}

pub type AgentResult = Result<AgentOutput, AgentError>;

/// Why a call changed nothing (or, when it says so, only part of what it
/// meant to). The message is what the agent reads.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum AgentError {
    #[error("no map is open")]
    NoMapOpen,
    #[error("editor is busy: {0}")]
    Busy(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("out of bounds: {0}")]
    OutOfBounds(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("refused: {0}")]
    Guarded(String),
}
