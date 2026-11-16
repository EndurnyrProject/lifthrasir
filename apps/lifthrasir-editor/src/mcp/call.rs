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
    PlaceAsset(PlaceParams),
    AddLight(AddLightParams),
    UpdateNode(Box<UpdateNodeParams>),
    DeleteNode(NodeRef),
    SetMapSettings(MapSettingsParams),
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
            Call::PlaceAsset(_) => "place_asset",
            Call::AddLight(_) => "add_light",
            Call::UpdateNode(_) => "update_node",
            Call::DeleteNode(_) => "delete_node",
            Call::SetMapSettings(_) => "set_map_settings",
        }
    }
}

/// A placeable asset: a prop model, a sound or an effect (see list_assets).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssetParam {
    /// A prop model, `ro://models/...glb`.
    Model { uri: String },
    /// A sound emitter, `ro://data/wav/...wav`.
    Sound { uri: String },
    /// An effect emitter, by effect catalog id.
    Effect { id: u32 },
}

/// Where to put a new node: exactly one of `cell` and `world`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceParams {
    pub asset: AssetParam,
    /// Stand it on the walk surface at the centre of this GAT cell [x, y].
    #[serde(default)]
    pub cell: Option<[u32; 2]>,
    /// Or put it at this exact world point [x, y, z] (y is up).
    #[serde(default)]
    pub world: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LightKindParam {
    Point,
    Spot,
}

/// A new point or spot light with the map's default values.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddLightParams {
    pub kind: LightKindParam,
    /// Above the centre of this GAT cell [x, y]; exactly one of `cell`, `world`.
    #[serde(default)]
    pub cell: Option<[u32; 2]>,
    /// Or at this exact world point [x, y, z] (y is up).
    #[serde(default)]
    pub world: Option<[f32; 3]>,
    /// With `cell`: world units above the walk surface; default 10.
    #[serde(default)]
    pub height: Option<f32>,
}

/// Changes to one node; omitted fields stay as they are.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct UpdateNodeParams {
    pub node_id: u64,
    #[serde(default)]
    pub name: Option<String>,
    /// Move onto the walk surface at the centre of this GAT cell [x, y].
    #[serde(default)]
    pub cell: Option<[u32; 2]>,
    /// Or move to this exact world point [x, y, z]; not with `cell`.
    #[serde(default)]
    pub translation: Option<[f32; 3]>,
    /// Rotation as XYZ Euler angles in degrees (props and spot lights).
    #[serde(default)]
    pub rotation_deg: Option<[f32; 3]>,
    /// Scale per axis (props only).
    #[serde(default)]
    pub scale: Option<[f32; 3]>,
    #[serde(default)]
    pub prop: Option<PropPatch>,
    #[serde(default)]
    pub sound: Option<SoundPatch>,
    #[serde(default)]
    pub effect: Option<EffectPatch>,
    /// Light fields (point, spot or sun).
    #[serde(default)]
    pub light: Option<LightPatch>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct PropPatch {
    /// A `ro://models/...glb` model from list_assets.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub anim_type: Option<u32>,
    #[serde(default)]
    pub anim_speed: Option<f32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SoundPatch {
    /// A `ro://data/wav/...wav` file from list_assets.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub volume: Option<f32>,
    /// Hearing range in world units.
    #[serde(default)]
    pub range: Option<f32>,
    /// Seconds between repeats.
    #[serde(default)]
    pub cycle: Option<f32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct EffectPatch {
    /// An effect catalog id from list_assets.
    #[serde(default)]
    pub effect_type: Option<u32>,
    #[serde(default)]
    pub emit_speed: Option<f32>,
    #[serde(default)]
    pub params: Option<[f32; 4]>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct LightPatch {
    /// sRGB color, each channel 0..=1.
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    /// Point and spot lights: intensity in candela.
    #[serde(default)]
    pub candela: Option<f32>,
    /// Point and spot lights: range in world units.
    #[serde(default)]
    pub range: Option<f32>,
    /// Spot lights: inner cone angle, 0 < inner <= outer <= 90.
    #[serde(default)]
    pub inner_angle_deg: Option<f32>,
    /// Spot lights: outer cone angle.
    #[serde(default)]
    pub outer_angle_deg: Option<f32>,
    /// Sun: illuminance in lux.
    #[serde(default)]
    pub lux: Option<f32>,
    /// Sun: angle above the horizon, 0..=90 degrees.
    #[serde(default)]
    pub elevation_deg: Option<f32>,
    /// Sun: compass direction in degrees.
    #[serde(default)]
    pub azimuth_deg: Option<f32>,
}

/// Map-wide lighting; omitted fields stay as they are.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct MapSettingsParams {
    /// Indoor maps; switching also applies the indoor/outdoor ambient and
    /// exposure presets, which explicit values below then override.
    #[serde(default)]
    pub indoor: Option<bool>,
    /// Ambient light color, RGB 0..=1.
    #[serde(default)]
    pub ambient_color: Option<[f32; 3]>,
    /// Ambient brightness in lux.
    #[serde(default)]
    pub ambient_brightness: Option<f32>,
    /// Camera exposure (EV100).
    #[serde(default)]
    pub exposure_ev100: Option<f32>,
    /// Tint applied to props' unshaded side, RGB 0..=1.
    #[serde(default)]
    pub no_shade_tint: Option<[f32; 3]>,
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
