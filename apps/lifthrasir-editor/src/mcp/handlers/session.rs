//! Editor state, undo and redo.

use super::kind_name;
use crate::{
    map::open::{EditorOperation, EditorScreen, ErrorModal, OpenMap},
    mcp::{
        call::{AgentError, AgentOutput, AgentResult},
        coords::map_bounds,
    },
    shell::{
        node::{find_node, kind_of},
        selection::Selection,
        undo::{self, UndoStack},
    },
};
use bevy::prelude::*;
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
