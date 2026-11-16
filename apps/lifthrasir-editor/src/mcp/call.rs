//! What an agent call carries into the Bevy world and what comes back. No MCP
//! protocol types live here, so handlers are testable without a server.

use schemars::JsonSchema;
use serde::Deserialize;
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
    ListMaps,
    OpenMap(OpenMapParams),
    NewMap(NewMapToolParams),
    SaveMap,
    MapSummary,
    QueryRegion(RegionQuery),
    ListNodes(ListNodesParams),
    GetNode(NodeRef),
    ListAssets(ListAssetsParams),
}

impl Call {
    /// The MCP tool name; also the undo label suffix (`Agent: <name>`).
    pub fn name(&self) -> &'static str {
        match self {
            Call::Status => "editor_status",
            Call::Undo => "undo",
            Call::Redo => "redo",
            Call::ListMaps => "list_maps",
            Call::OpenMap(_) => "open_map",
            Call::NewMap(_) => "new_map",
            Call::SaveMap => "save_map",
            Call::MapSummary => "map_summary",
            Call::QueryRegion(_) => "query_region",
            Call::ListNodes(_) => "list_nodes",
            Call::GetNode(_) => "get_node",
            Call::ListAssets(_) => "list_assets",
        }
    }
}

/// A rectangle of GAT cells, inclusive; at most 64 x 64.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegionQuery {
    /// The corner with the smallest x and y, as a GAT cell [x, y].
    pub min: [u32; 2],
    /// The corner with the largest x and y, as a GAT cell [x, y].
    pub max: [u32; 2],
}

/// Node kinds as the tools name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKindParam {
    Prop,
    Sound,
    Effect,
    PointLight,
    SpotLight,
    Sun,
}

/// Which nodes to list; every filter is optional.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListNodesParams {
    /// Only nodes of this kind.
    #[serde(default)]
    pub kind: Option<NodeKindParam>,
    /// With `max`: only nodes standing on a GAT cell in this rectangle.
    #[serde(default)]
    pub min: Option<[u32; 2]>,
    /// With `min`: only nodes standing on a GAT cell in this rectangle.
    #[serde(default)]
    pub max: Option<[u32; 2]>,
}

/// One node, by the id list_nodes reports.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NodeRef {
    /// The node id from list_nodes, get_node or the call that created it.
    pub node_id: u64,
}

/// The asset catalogs.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetKindParam {
    /// Prop models (`ro://models/...glb`), placeable with place_asset.
    Model,
    /// Ground textures (`ro://ground/tex/...ktx2`), for paint_tiles and new_map.
    GroundTexture,
    /// Sounds (`ro://data/wav/...wav`), placeable with place_asset.
    Sound,
    /// Effect ids from the effect catalog, placeable with place_asset.
    Effect,
}

/// A slice of one asset catalog.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAssetsParams {
    /// Which catalog to read.
    pub kind: AssetKindParam,
    /// Case-sensitive substring the asset must contain (paths are lowercase).
    #[serde(default)]
    pub filter: Option<String>,
    /// At most this many results; default 100.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenMapParams {
    /// The map to open, as `ro://maps/<name>/<name>.glb` (see list_maps).
    pub path: String,
}

/// A new flat map; sculpt it afterwards.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NewMapToolParams {
    /// Lowercase map name: a-z, 0-9, `_`, `-`, `@`. Must not exist yet.
    pub name: String,
    /// Width in GAT cells; even and greater than zero.
    pub width: u32,
    /// Height in GAT cells; even and greater than zero.
    pub height: u32,
    /// The ground texture covering the whole map, a `ro://ground/tex/...ktx2`
    /// URI from list_assets.
    pub texture: String,
    /// Lighting and exposure preset.
    pub preset: PresetParam,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PresetParam {
    Outdoor,
    Indoor,
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
