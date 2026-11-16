use crate::{
    map::{
        layout::MapLayout,
        tools::{
            gat::GAT_CELL_SIZE,
            sculpt::{self, SculptBrush, SculptOp},
        },
    },
    mcp::{
        call::{AgentError, AgentOutput, AgentResult, SculptOpParam, SculptParams},
        coords::{self, Region},
    },
    shell::undo::UndoStack,
};
use bevy::prelude::*;
use lifthrasir_runtime::world::LifMapData;
use serde_json::json;

fn brush_hit(world: &World, center: [u32; 2], radius: u32) -> Result<Vec3, AgentError> {
    let bounds = coords::map_bounds(world)?;
    coords::region_cells(&Region::Circle { center, radius }, bounds)?;
    let root = world.resource::<MapLayout>().root;
    let altitude = &world
        .get::<LifMapData>(root)
        .ok_or(AgentError::NoMapOpen)?
        .altitude;
    coords::cell_center(altitude, coords::from_wire(center))
}

pub fn sculpt(world: &mut World, p: SculptParams) -> AgentResult {
    if !sculpt::STRENGTH_RANGE.contains(&p.strength) {
        return Err(AgentError::Invalid("invalid sculpt strength".into()));
    }
    let hit = brush_hit(world, p.center, p.radius_cells)?;
    let op = match p.op {
        SculptOpParam::Raise => SculptOp::Raise,
        SculptOpParam::Lower => SculptOp::Lower,
        SculptOpParam::Smooth => SculptOp::Smooth,
        SculptOpParam::Flatten => SculptOp::Flatten,
    };
    let depth = world.resource::<UndoStack>().depth();
    sculpt::begin_stroke(world, hit);
    sculpt::sculpt_step(
        world,
        hit,
        SculptBrush {
            op,
            radius: p.radius_cells as f32 * GAT_CELL_SIZE,
            strength: p.strength,
        },
        1.0,
    );
    sculpt::end_stroke(world);
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}

pub fn paint_tiles(world: &mut World, p: crate::mcp::call::PaintTilesParams) -> AgentResult {
    use crate::{
        map::tools::paint::{self, TileBrush},
        shell::browser::AssetIndex,
    };
    let bounds = coords::map_bounds(world)? / 2;
    if !world
        .resource::<AssetIndex>()
        .ground_textures
        .contains(&p.texture)
    {
        return Err(AgentError::NotFound(p.texture));
    }
    if p.rotation > 3 {
        return Err(AgentError::Invalid("rotation must be 0..=3".into()));
    }
    let tiles = p
        .tiles
        .into_iter()
        .map(|tile| coords::in_bounds(coords::from_wire(tile), bounds))
        .collect::<Result<Vec<_>, _>>()?;
    let depth = world.resource::<UndoStack>().depth();
    let brush = TileBrush {
        texture_uri: p.texture,
        rotation: p.rotation,
        flip: p.flip,
    };
    paint::begin_stroke(world);
    for tile in tiles {
        paint::paint_cell(world, coords::to_tuple(tile), &brush);
    }
    paint::end_stroke(world);
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}

pub fn add_wall(world: &mut World, p: crate::mcp::call::WallParams) -> AgentResult {
    use crate::{
        map::tools::wall::{self, WallBrush},
        shell::browser::AssetIndex,
    };
    coords::map_bounds(world)?;
    if !world
        .resource::<AssetIndex>()
        .ground_textures
        .contains(&p.texture)
    {
        return Err(AgentError::NotFound(p.texture));
    }
    let depth = world.resource::<UndoStack>().depth();
    wall::add_wall(
        world,
        coords::to_tuple(coords::from_wire(p.a)),
        coords::to_tuple(coords::from_wire(p.b)),
        &WallBrush {
            texture_uri: p.texture,
        },
    )
    .map_err(AgentError::Invalid)?;
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}

pub fn paint_vertex_color(world: &mut World, p: crate::mcp::call::ColorParams) -> AgentResult {
    use crate::map::tools::color::{self, ColorBrush};
    if !color::STRENGTH_RANGE.contains(&p.strength) {
        return Err(AgentError::Invalid("invalid color strength".into()));
    }
    if !p
        .color
        .iter()
        .all(|component| (0.0..=1.0).contains(component))
    {
        return Err(AgentError::Invalid("invalid color component".into()));
    }
    let hit = brush_hit(world, p.center, p.radius_cells)?;
    let depth = world.resource::<UndoStack>().depth();
    color::begin_stroke(world);
    color::color_step(
        world,
        hit,
        ColorBrush {
            color: p.color,
            radius: p.radius_cells as f32 * GAT_CELL_SIZE,
            strength: p.strength,
        },
        1.0,
    );
    color::end_stroke(world);
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}

pub fn paint_water(world: &mut World, p: crate::mcp::call::WaterParams) -> AgentResult {
    use crate::map::tools::water;
    let bounds = coords::map_bounds(world)? / 2;
    let tiles = p
        .tiles
        .into_iter()
        .map(|tile| coords::in_bounds(coords::from_wire(tile), bounds))
        .collect::<Result<Vec<_>, _>>()?;
    let root = world.resource::<MapLayout>().root;
    let data = world.get::<LifMapData>(root).ok_or(AgentError::NoMapOpen)?;
    if data.water.is_none() && !p.add {
        return Err(AgentError::Invalid(
            "cannot remove water from a dry map".into(),
        ));
    }
    let depth = world.resource::<UndoStack>().depth();
    if data.water.is_none() {
        water::add_water(world);
    }
    let data = world.get::<LifMapData>(root).ok_or(AgentError::NoMapOpen)?;
    let water = data.water.clone();
    let tiles = water::paint_tiles(
        &data.water_tiles,
        tiles.into_iter().map(coords::to_usize),
        p.add,
    );
    water::commit_water(world, water, tiles);
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}

pub fn paint_gat(world: &mut World, p: crate::mcp::call::GatParams) -> AgentResult {
    use crate::map::tools::gat;
    let cells = coords::region_cells(&p.region, coords::map_bounds(world)?)?;
    if !gat::CELL_TYPES.contains(&p.raw_type) {
        return Err(AgentError::Invalid("unknown GAT cell type".into()));
    }
    let cells: Vec<_> = cells.into_iter().map(coords::to_tuple).collect();
    let depth = world.resource::<UndoStack>().depth();
    gat::paint_region(world, &cells, p.raw_type);
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}

pub fn auto_gat(world: &mut World, p: crate::mcp::call::AutoGatParams) -> AgentResult {
    use crate::map::tools::gat::{self, AutoGatOptions};
    coords::map_bounds(world)?;
    let depth = world.resource::<UndoStack>().depth();
    gat::auto_gat(
        world,
        AutoGatOptions {
            set_heights: p.set_heights,
            set_walkability: p.set_walkability,
            block_under_water: p.block_under_water,
            block_props: p.block_props,
        },
    );
    Ok(AgentOutput::json(
        json!({"changed": world.resource::<UndoStack>().depth() > depth}),
    ))
}
