//! One handler per tool. Each validates everything before its first change.

pub mod capture;
pub mod objects;
pub mod query;
pub mod session;
pub mod terrain;

use super::call::{AgentError, AgentResult, Call};
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
        Call::PaintWater(p) => terrain::paint_water(world, p),
        Call::PaintGat(p) => terrain::paint_gat(world, p),
        Call::AutoGat(p) => terrain::auto_gat(world, p),
        Call::PaintTiles(p) => terrain::paint_tiles(world, p),
        Call::AddWall(p) => terrain::add_wall(world, p),
        Call::PaintVertexColor(p) => terrain::paint_vertex_color(world, p),
        Call::Sculpt(p) => terrain::sculpt(world, p),
        Call::Status => session::editor_status(world),
        Call::Undo => session::undo(world),
        Call::Redo => session::redo(world),
        Call::ListMaps => session::list(world),
        Call::OpenMap(p) => session::open(world, p),
        Call::NewMap(p) => session::new_map(world, p),
        Call::SaveMap => session::save_map(world),
        Call::MapSummary => query::map_summary(world),
        Call::QueryRegion(p) => query::query_region(world, p),
        Call::ListNodes(p) => query::list_nodes(world, p),
        Call::GetNode(p) => query::get_node(world, p),
        Call::ListAssets(p) => query::list_assets(world, p),
        Call::PlaceAsset(p) => objects::place_asset(world, p),
        Call::AddLight(p) => objects::add_light(world, p),
        Call::UpdateNode(p) => objects::update_node(world, *p),
        Call::DeleteNode(p) => objects::delete_node(world, p),
        Call::SetMapSettings(p) => objects::set_map_settings(world, p),
        Call::Screenshot | Call::RenderTopDown(_) => Err(AgentError::Invalid(
            "captures reply from the render; dispatch starts them".into(),
        )),
    }
}
