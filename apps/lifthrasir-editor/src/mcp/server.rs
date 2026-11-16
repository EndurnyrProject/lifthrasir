//! The MCP server: rmcp over streamable HTTP on `127.0.0.1`. Every tool turns
//! into a [`Call`] for the Bevy world and waits for its reply; rmcp's default
//! config accepts only loopback `Host` headers (DNS rebinding protection).

use super::call::{
    AddLightParams, AgentCall, AgentOutput, AgentResult, Call, ListAssetsParams, ListNodesParams,
    MapSettingsParams, NewMapToolParams, NodeRef, OpenMapParams, PlaceParams, RegionQuery,
    TopDownParams, UpdateNodeParams,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
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

    #[tool(description = "Every map the editor can open, by name.")]
    async fn list_maps(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::ListMaps).await
    }

    #[tool(
        description = "Start opening a map (ro://maps/<name>/<name>.glb). Refused while there \
        are unsaved edits. Returns at once; poll editor_status until operation is idle and \
        map is set, or last_error explains a failure."
    )]
    async fn open_map(
        &self,
        Parameters(p): Parameters<OpenMapParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::OpenMap(p)).await
    }

    #[tool(
        description = "Create a new flat, fully walkable map with one ground texture and open \
        it. Refused while there are unsaved edits or when the name exists. Poll editor_status \
        until it is open."
    )]
    async fn new_map(
        &self,
        Parameters(p): Parameters<NewMapToolParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::NewMap(p)).await
    }

    #[tool(
        description = "Save the open map under its own name. Validation errors (missing \
        assets, invalid data) come back as the tool error."
    )]
    async fn save_map(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::SaveMap).await
    }

    #[tool(
        description = "Summary of the open map: name, size in cells and tiles, indoor flag, \
        ambient and exposure, water zones, node counts by kind and the ground textures used."
    )]
    async fn map_summary(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::MapSummary).await
    }

    #[tool(
        description = "Per GAT cell of a rectangle (at most 64 x 64 cells): the 4 GAT corner \
        heights, walk_y (world height at the cell centre), the GAT type (raw + label), the \
        ground texture of its tile and whether its tile has water."
    )]
    async fn query_region(
        &self,
        Parameters(p): Parameters<RegionQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::QueryRegion(p)).await
    }

    #[tool(
        description = "Nodes (props, sounds, effects, lights, sun) with id, name, kind, GAT \
        cell and world translation; optionally of one kind or inside a cell rectangle."
    )]
    async fn list_nodes(
        &self,
        Parameters(p): Parameters<ListNodesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::ListNodes(p)).await
    }

    #[tool(
        description = "One node: transform (translation, rotation in degrees, scale), GAT cell \
        and payload (prop model, sound, effect, or light color/candela/range/angles)."
    )]
    async fn get_node(
        &self,
        Parameters(p): Parameters<NodeRef>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::GetNode(p)).await
    }

    #[tool(
        description = "Valid asset references to use in other tools: models, ground textures, \
        sounds (ro:// URIs) or effect ids, filtered by substring; returns total and up to \
        limit (default 100)."
    )]
    async fn list_assets(
        &self,
        Parameters(p): Parameters<ListAssetsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::ListAssets(p)).await
    }

    #[tool(
        description = "Place a prop model, sound or effect (see list_assets) on the walk \
        surface of a GAT cell, or at a world point. Returns its node_id."
    )]
    async fn place_asset(
        &self,
        Parameters(p): Parameters<PlaceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::PlaceAsset(p)).await
    }

    #[tool(
        description = "Add a point or spot light with the map's defaults, 10 units above a \
        GAT cell (or `height`), or at a world point. Returns its node_id."
    )]
    async fn add_light(
        &self,
        Parameters(p): Parameters<AddLightParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::AddLight(p)).await
    }

    #[tool(
        description = "Change one node: name, position (cell or world), rotation (props, spot \
        lights), scale (props), prop/sound/effect fields, or light fields (color, candela, \
        range, cone angles; sun: lux, elevation, azimuth). Omitted fields stay."
    )]
    async fn update_node(
        &self,
        Parameters(p): Parameters<UpdateNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::UpdateNode(Box::new(p))).await
    }

    #[tool(description = "Delete one node. The sun cannot be deleted.")]
    async fn delete_node(
        &self,
        Parameters(p): Parameters<NodeRef>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::DeleteNode(p)).await
    }

    #[tool(
        description = "Change map-wide lighting: indoor flag (applies its presets), ambient \
        color and brightness, exposure, prop shade tint. Omitted fields stay."
    )]
    async fn set_map_settings(
        &self,
        Parameters(p): Parameters<MapSettingsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::SetMapSettings(p)).await
    }

    #[tool(
        description = "A 1280x720 PNG of what the editor camera sees right now (without the \
        editor's panels). Use it to judge how an area looks."
    )]
    async fn screenshot(&self) -> Result<CallToolResult, ErrorData> {
        self.call(Call::Screenshot).await
    }

    #[tool(
        description = "A PNG of a GAT rectangle seen from straight above, orthographic, north \
        (+y) at the top and +x to the right. px_per_cell in the reply maps pixels to cells; \
        pixel (0, 0) is cell [min.x, max.y]. Use it to check placement and painting."
    )]
    async fn render_top_down(
        &self,
        Parameters(p): Parameters<TopDownParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(Call::RenderTopDown(p)).await
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
        assert_eq!(
            names,
            [
                "add_light",
                "delete_node",
                "editor_status",
                "get_node",
                "list_assets",
                "list_maps",
                "list_nodes",
                "map_summary",
                "new_map",
                "open_map",
                "place_asset",
                "query_region",
                "redo",
                "render_top_down",
                "save_map",
                "screenshot",
                "set_map_settings",
                "undo",
                "update_node"
            ]
        );
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
