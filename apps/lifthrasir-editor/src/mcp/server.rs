//! The MCP server: rmcp over streamable HTTP on `127.0.0.1`. Every tool turns
//! into a [`Call`] for the Bevy world and waits for its reply; rmcp's default
//! config accepts only loopback `Host` headers (DNS rebinding protection).

use super::call::{AgentCall, AgentOutput, AgentResult, Call};
use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

/// What an agent reads once, on connect.
pub const INSTRUCTIONS: &str = "Lifthrasir map editor, live: a human edits the same map \
while you work, and every edit you make is one undo entry they can revert.\n\
Grids: `cell` means a GAT cell [x, y] (walkability/height grid); `tile` means a ground \
tile, which covers 2 x 2 cells (tile = cell / 2). y grows north. editor_status reports \
both sizes.\n\
Nodes (props, sounds, effects, lights, sun) have numeric ids, stable until the map is \
reopened; use list_nodes to find them again.\n\
Loop: query the area, make one edit, then render it to check before moving on.\n\
Save with save_map when done. open_map and new_map are refused while there are \
unsaved edits.";

/// The MCP service: one per session, all sending into the same world.
#[derive(Clone)]
pub struct EditorMcp {
    tx: UnboundedSender<AgentCall>,
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl EditorMcp {
    pub fn new(tx: UnboundedSender<AgentCall>) -> Self {
        Self {
            tx,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "The open map (name, size in cells and tiles), the current operation \
        (idle/loading/saving), unsaved changes, undo/redo labels, the last error the human \
        saw, and the node the human has selected."
    )]
    async fn editor_status(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::Status).await
    }

    #[tool(
        description = "Undo the newest entry in the shared undo history (yours or the human's)."
    )]
    async fn undo(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::Undo).await
    }

    #[tool(description = "Redo the entry undo last reverted.")]
    async fn redo(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::Redo).await
    }
}

impl EditorMcp {
    /// Sends `call` to the world and turns its reply into a tool result.
    async fn call(&self, call: Call) -> Result<CallToolResult, ErrorData> {
        let (reply, result) = oneshot::channel();
        self.tx
            .send(AgentCall { call, reply })
            .map_err(|_| ErrorData::internal_error("the editor has shut down", None))?;
        let result = result
            .await
            .map_err(|_| ErrorData::internal_error("the editor dropped the call", None))?;
        Ok(tool_result(result))
    }
}

/// An agent error is a tool error the agent reads, not a protocol error.
pub fn tool_result(result: AgentResult) -> CallToolResult {
    let AgentOutput { json, png } = match result {
        Ok(output) => output,
        Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
    };
    let mut content = vec![ContentBlock::text(json.to_string())];
    if let Some(png) = png {
        content.push(ContentBlock::image(STANDARD.encode(png), "image/png"));
    }
    CallToolResult::success(content)
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EditorMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "lifthrasir-editor",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Serves MCP on `127.0.0.1:<port>/mcp` until the process exits.
pub async fn serve(port: u16, tx: UnboundedSender<AgentCall>) -> anyhow::Result<()> {
    let service = StreamableHttpService::new(
        move || Ok(EditorMcp::new(tx.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|error| anyhow::anyhow!("binding 127.0.0.1:{port}: {error}"))?;
    bevy::log::info!("MCP server on http://127.0.0.1:{port}/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::call::AgentError;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn server_lists_every_tool() {
        let (tx, _rx) = unbounded_channel();
        let tools = EditorMcp::new(tx).tool_router.list_all();
        let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        names.sort();
        assert_eq!(names, ["editor_status", "redo", "undo"]);
        for tool in &tools {
            assert_eq!(
                tool.input_schema.get("type"),
                Some(&"object".into()),
                "{}",
                tool.name
            );
        }
    }

    #[test]
    fn agent_errors_become_tool_errors() {
        let result = tool_result(Err(AgentError::NoMapOpen));
        assert_eq!(result.is_error, Some(true));
        let text = result.content[0].as_text().map(|text| text.text.as_str());
        assert_eq!(text, Some("no map is open"));
    }
}
