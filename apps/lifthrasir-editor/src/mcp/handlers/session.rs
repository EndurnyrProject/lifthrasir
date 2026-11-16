//! Editor state, maps (list, open, new, save), undo and redo.

use super::kind_name;
use crate::{
    EditorPaths,
    map::{
        layout::map_name_from_path,
        open::{EditorOperation, EditorScreen, ErrorModal, OpenMap, OpenMapRequest, list_maps},
        save::save,
        wizard::{NewMapWizard, create},
    },
    mcp::{
        call::{
            AgentError, AgentOutput, AgentResult, NewMapToolParams, OpenMapParams, PresetParam,
        },
        coords::map_bounds,
    },
    shell::{
        node::{find_node, kind_of},
        selection::Selection,
        undo::{self, UndoStack},
    },
};
use bevy::prelude::*;
use lifthrasir_assets::source::{AssetSource, SharedCompositeAssetSource};
use lifthrasir_tooling::converters::map::blank::LightingPreset;
use serde_json::{Value, json};

/// The open map, the current operation, history labels, the last error the
/// human saw and the node they have selected.
pub fn editor_status(world: &mut World) -> AgentResult {
    let operation = match world.resource::<EditorOperation>() {
        EditorOperation::Idle => "idle",
        EditorOperation::Loading { .. } => "loading",
        EditorOperation::Saving { .. } => "saving",
        EditorOperation::Saved { .. } => "saved",
    };
    let stack = world.resource::<UndoStack>();
    let errors = world.resource::<ErrorModal>();
    let last_error = (!errors.messages.is_empty())
        .then(|| json!({ "title": errors.title, "messages": errors.messages }));
    Ok(AgentOutput::json(json!({
        "map": open_map(world),
        "operation": operation,
        "dirty": stack.is_dirty(),
        "undo_label": stack.undo_label(),
        "redo_label": stack.redo_label(),
        "last_error": last_error,
        "selected_node": selected_node(world),
    })))
}

pub fn undo(world: &mut World) -> AgentResult {
    if world.resource::<UndoStack>().undo_label().is_none() {
        return Err(AgentError::Invalid("nothing to undo".into()));
    }
    undo::undo(world);
    Ok(history(world))
}

pub fn redo(world: &mut World) -> AgentResult {
    if world.resource::<UndoStack>().redo_label().is_none() {
        return Err(AgentError::Invalid("nothing to redo".into()));
    }
    undo::redo(world);
    Ok(history(world))
}

fn history(world: &World) -> AgentOutput {
    let stack = world.resource::<UndoStack>();
    AgentOutput::json(json!({
        "undo_label": stack.undo_label(),
        "redo_label": stack.redo_label(),
    }))
}

/// Every map name the source holds, sorted.
pub fn list(world: &mut World) -> AgentResult {
    let source = world.resource::<SharedCompositeAssetSource>().0.clone();
    let source = source
        .read()
        .map_err(|_| AgentError::Invalid("the ro:// source lock is poisoned".into()))?;
    Ok(AgentOutput::json(json!({ "maps": list_maps(&*source) })))
}

/// Starts opening `path`; the agent polls `editor_status` until it is editing.
pub fn open(world: &mut World, p: OpenMapParams) -> AgentResult {
    refuse_if_dirty(world)?;
    if map_name_from_path(&p.path).is_none() {
        return Err(AgentError::Invalid(format!(
            "'{}' is not a ro://maps/<name>/<name>.glb path",
            p.path
        )));
    }
    let exists = {
        let source = world.resource::<SharedCompositeAssetSource>().0.clone();
        let source = source
            .read()
            .map_err(|_| AgentError::Invalid("the ro:// source lock is poisoned".into()))?;
        source.exists(p.path.trim_start_matches("ro://"))
    };
    if !exists {
        return Err(AgentError::NotFound(p.path));
    }
    world.write_message(OpenMapRequest { path: p.path });
    Ok(AgentOutput::json(json!({ "started": true })))
}

/// Writes a new flat map and starts opening it, like the New map wizard.
pub fn new_map(world: &mut World, p: NewMapToolParams) -> AgentResult {
    refuse_if_dirty(world)?;
    super::require_ground_texture(world, &p.texture)?;
    let wizard = NewMapWizard {
        name: p.name,
        width: p.width,
        height: p.height,
        texture_uri: Some(p.texture),
        heightmap: String::new(),
        preset: match p.preset {
            PresetParam::Outdoor => LightingPreset::Outdoor,
            PresetParam::Indoor => LightingPreset::Indoor,
        },
        ..default()
    };
    let data_folder = world.resource::<EditorPaths>().data_folder.clone();
    let path = {
        let source = world.resource::<SharedCompositeAssetSource>().0.clone();
        let source = source
            .read()
            .map_err(|_| AgentError::Invalid("the ro:// source lock is poisoned".into()))?;
        create(&wizard, &*source, &data_folder).map_err(AgentError::Invalid)?
    };
    world.write_message(OpenMapRequest { path: path.clone() });
    Ok(AgentOutput::json(json!({ "started": true, "path": path })))
}

/// Saves the open map under its own name. Failures go to the agent, not to
/// the human's error modal.
pub fn save_map(world: &mut World) -> AgentResult {
    if world.get_resource::<OpenMap>().is_none() {
        return Err(AgentError::NoMapOpen);
    }
    save(world, None).map_err(|messages| {
        warn!("agent save failed: {}", messages.join("; "));
        AgentError::Invalid(messages.join("; "))
    })?;
    let path = world.resource::<OpenMap>().path.clone();
    Ok(AgentOutput::json(json!({ "saved": true, "path": path })))
}

fn refuse_if_dirty(world: &World) -> Result<(), AgentError> {
    if world.resource::<UndoStack>().is_dirty() {
        return Err(AgentError::Guarded(
            "unsaved changes; save or undo first".into(),
        ));
    }
    Ok(())
}

/// The map being edited; `null` on the start screen and while loading.
fn open_map(world: &World) -> Value {
    let editing = world
        .get_resource::<State<EditorScreen>>()
        .is_some_and(|screen| *screen.get() == EditorScreen::Editing);
    let (Some(open), Ok(cells)) = (world.get_resource::<OpenMap>(), map_bounds(world)) else {
        return Value::Null;
    };
    if !editing {
        return Value::Null;
    }
    json!({
        "name": open.name,
        "path": open.path,
        "cells": [cells.x, cells.y],
        "tiles": [cells.x / 2, cells.y / 2],
    })
}

fn selected_node(world: &mut World) -> Value {
    let Some(id) = world.resource::<Selection>().0 else {
        return Value::Null;
    };
    let Some(kind) = find_node(world, id).and_then(|node| kind_of(world, node)) else {
        return Value::Null;
    };
    json!({ "id": id.0, "kind": kind_name(kind) })
}
