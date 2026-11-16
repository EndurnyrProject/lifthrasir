//! Adding, editing and deleting map objects, and map-wide lighting. Every
//! input is validated before the first edit; dispatch folds the edits of one
//! call into one undo entry. Nothing here touches selection or tool mode,
//! except that deleting the selected node clears the selection.

use super::{
    kind_name,
    query::{find_node_or_err, node_dto},
};
use crate::{
    map::{
        layout::MapLayout,
        open::OpenMap,
        refresh::after_node_edit,
        settings::{commit_settings, set_indoor},
        tools::{
            gat::gat_cell_at,
            lights::{LIGHT_HEIGHT, LightKind, spawn_light, spot_angles_valid},
            place::spawn_placed,
        },
    },
    mcp::{
        call::{
            AddLightParams, AgentError, AgentOutput, AgentResult, AssetParam, LightKindParam,
            LightPatch, MapSettingsParams, NodeRef, PlaceParams, UpdateNodeParams,
        },
        coords::{cell_center, from_wire},
    },
    shell::{
        browser::{AssetIndex, AssetRef},
        gizmo::allowed_modes,
        inspector::with_sun_elevation,
        node::{EditorNodeId, NodeKind, NodeSnapshot, kind_of},
        selection::Selection,
        undo::{ComponentEdit, DespawnNode, EditOp, commit, component_holder},
    },
};
use bevy::{prelude::*, utils::prelude::ShortName};
use lifthrasir_assets::source::{AssetSource, SharedCompositeAssetSource};
use lifthrasir_runtime::world::{
    LifAudioEmitter, LifEffectEmitter, LifMapData, LifPropRef, MapLightCandela,
};
use serde_json::json;
use transform_gizmo::config::GizmoModeKind;

/// Adds a prop, sound or effect at a cell or world point.
pub fn place_asset(world: &mut World, p: PlaceParams) -> AgentResult {
    let data = open_data(world)?;
    let asset = asset_ref(world, p.asset)?;
    let at = position(&data, p.cell, p.world, 0.0)?;
    let id = spawn_placed(world, &asset, at)
        .ok_or_else(|| AgentError::Invalid("the asset cannot be placed".into()))?;
    Ok(placed(id, at, &data))
}

/// Adds a point or spot light above a cell, or at a world point.
pub fn add_light(world: &mut World, p: AddLightParams) -> AgentResult {
    let data = open_data(world)?;
    let lift = p.height.unwrap_or(LIGHT_HEIGHT);
    let at = position(&data, p.cell, p.world, lift)?;
    let kind = match p.kind {
        LightKindParam::Point => LightKind::Point,
        LightKindParam::Spot => LightKind::Spot,
    };
    let id = spawn_light(world, kind, at).ok_or(AgentError::NoMapOpen)?;
    Ok(placed(id, at, &data))
}

/// Applies the given fields to one node.
pub fn update_node(world: &mut World, p: UpdateNodeParams) -> AgentResult {
    let data = open_data(world)?;
    let node = find_node_or_err(world, p.node_id)?;
    let id = EditorNodeId(p.node_id);
    let kind =
        kind_of(world, node).ok_or_else(|| AgentError::NotFound(format!("node {}", p.node_id)))?;
    let mut edits: Vec<Box<dyn EditOp>> = Vec::new();
    if let Some(name) = p.name.clone() {
        edits.push(edit(world, id, node, "name", |n: &mut Name| {
            *n = Name::new(name)
        })?);
    }
    if let Some(after) = transform_after(world, node, kind, &p, &data)? {
        edits.push(edit(world, id, node, "transform", |t: &mut Transform| {
            *t = after
        })?);
    }
    edits.extend(payload_edits(world, id, node, kind, &p)?);
    if let Some(light) = &p.light {
        edits.extend(light_edits(world, id, node, kind, light)?);
    }
    if edits.is_empty() {
        return Ok(AgentOutput::json(json!({ "changed": false })));
    }
    for op in edits {
        commit(world, op);
    }
    let node = find_node_or_err(world, p.node_id)?;
    let data = open_data(world)?;
    node_dto(world, node, &data)
        .map(AgentOutput::json)
        .ok_or_else(|| AgentError::NotFound(format!("node {}", p.node_id)))
}

/// Deletes one node; the sun cannot be deleted.
pub fn delete_node(world: &mut World, p: NodeRef) -> AgentResult {
    open_data(world)?;
    let node = find_node_or_err(world, p.node_id)?;
    if NodeSnapshot::capture(world, node).is_none() {
        return Err(AgentError::Invalid(format!(
            "node {} cannot be deleted (the sun stays)",
            p.node_id
        )));
    }
    let layout = world.resource::<MapLayout>().clone();
    let id = EditorNodeId(p.node_id);
    commit(
        world,
        Box::new(DespawnNode::new(layout.root, layout.map_node, id)),
    );
    let mut selection = world.resource_mut::<Selection>();
    if selection.0 == Some(id) {
        selection.0 = None;
    }
    Ok(AgentOutput::json(json!({ "deleted": p.node_id })))
}

/// Changes the map-wide lighting.
pub fn set_map_settings(world: &mut World, p: MapSettingsParams) -> AgentResult {
    let data = open_data(world)?;
    let mut meta = data.meta.clone();
    if let Some(indoor) = p.indoor {
        set_indoor(&mut meta, indoor);
    }
    if let Some(color) = p.ambient_color {
        meta.ambient_color = color;
    }
    if let Some(brightness) = p.ambient_brightness {
        meta.ambient_brightness = brightness;
    }
    if let Some(ev100) = p.exposure_ev100 {
        meta.exposure_ev100 = ev100;
    }
    if let Some(tint) = p.no_shade_tint {
        meta.no_shade_tint = tint;
    }
    if meta == data.meta {
        return Ok(AgentOutput::json(json!({ "changed": false })));
    }
    commit_settings(world, meta.clone());
    Ok(AgentOutput::json(json!({
        "indoor": meta.indoor,
        "ambient_color": meta.ambient_color,
        "ambient_brightness": meta.ambient_brightness,
        "exposure_ev100": meta.exposure_ev100,
        "no_shade_tint": meta.no_shade_tint,
    })))
}

/// The open map's data, or `NoMapOpen`.
fn open_data(world: &World) -> Result<LifMapData, AgentError> {
    world
        .get_resource::<OpenMap>()
        .ok_or(AgentError::NoMapOpen)?;
    let root = world
        .get_resource::<MapLayout>()
        .ok_or(AgentError::NoMapOpen)?
        .root;
    world
        .get::<LifMapData>(root)
        .cloned()
        .ok_or(AgentError::NoMapOpen)
}

/// Exactly one of a cell (on its walk surface, raised by `lift`) or a world point.
fn position(
    data: &LifMapData,
    cell: Option<[u32; 2]>,
    point: Option<[f32; 3]>,
    lift: f32,
) -> Result<Vec3, AgentError> {
    match (cell, point) {
        (Some(cell), None) => Ok(cell_center(&data.altitude, from_wire(cell))? + Vec3::Y * lift),
        (None, Some(point)) => Ok(Vec3::from_array(point)),
        _ => Err(AgentError::Invalid(
            "give exactly one of cell and world".into(),
        )),
    }
}

fn placed(id: EditorNodeId, at: Vec3, data: &LifMapData) -> AgentOutput {
    let cell = gat_cell_at(&data.altitude, at).map(|(x, y)| [x, y]);
    AgentOutput::json(json!({ "node_id": id.0, "cell": cell, "translation": at.to_array() }))
}

/// The asset as the place tool takes it, after checking it exists.
fn asset_ref(world: &World, asset: AssetParam) -> Result<AssetRef, AgentError> {
    match asset {
        AssetParam::Model { uri } => {
            require_file(world, &uri)?;
            Ok(AssetRef::Model(uri))
        }
        AssetParam::Sound { uri } => {
            require_file(world, &uri)?;
            Ok(AssetRef::Sound(uri))
        }
        AssetParam::Effect { id } => {
            require_effect(world, id)?;
            Ok(AssetRef::Effect(id))
        }
    }
}

fn require_file(world: &World, uri: &str) -> Result<(), AgentError> {
    let path = uri
        .strip_prefix("ro://")
        .ok_or_else(|| AgentError::Invalid(format!("'{uri}' is not a ro:// URI")))?;
    let source = world.resource::<SharedCompositeAssetSource>().0.clone();
    let exists = source
        .read()
        .map_err(|_| AgentError::Invalid("the ro:// source lock is poisoned".into()))?
        .exists(path);
    if !exists {
        return Err(AgentError::NotFound(uri.to_string()));
    }
    Ok(())
}

fn require_effect(world: &World, id: u32) -> Result<(), AgentError> {
    if !world.resource::<AssetIndex>().effects.contains(&id) {
        return Err(AgentError::NotFound(format!("effect {id}")));
    }
    Ok(())
}

/// One undoable change of component `T` on `node` (or its light child).
fn edit<T: Component + Clone>(
    world: &World,
    id: EditorNodeId,
    node: Entity,
    field: &str,
    change: impl FnOnce(&mut T),
) -> Result<Box<dyn EditOp>, AgentError> {
    let holder = component_holder::<T>(world, node);
    let before = world.get::<T>(holder).cloned().ok_or_else(|| {
        AgentError::Invalid(format!(
            "node {} has no {} for `{field}`",
            id.0,
            ShortName::of::<T>()
        ))
    })?;
    let mut after = before.clone();
    change(&mut after);
    Ok(Box::new(
        ComponentEdit::new(id, before, after).with_hook(after_node_edit),
    ))
}

/// The node's new transform, or `None` when no transform field was given.
/// Each part must be one the GUI gizmo allows for the node's kind.
fn transform_after(
    world: &World,
    node: Entity,
    kind: NodeKind,
    p: &UpdateNodeParams,
    data: &LifMapData,
) -> Result<Option<Transform>, AgentError> {
    let sun_angles = p
        .light
        .as_ref()
        .filter(|light| light.elevation_deg.is_some() || light.azimuth_deg.is_some());
    let moved = p.cell.is_some() || p.translation.is_some();
    if !moved && p.rotation_deg.is_none() && p.scale.is_none() && sun_angles.is_none() {
        return Ok(None);
    }
    let allowed = allowed_modes(kind);
    let refuse =
        |part: &str| AgentError::Invalid(format!("a {} cannot change its {part}", kind_name(kind)));
    let mut transform = *world
        .get::<Transform>(node)
        .ok_or_else(|| AgentError::NotFound("the node has no transform".into()))?;
    if moved {
        if !allowed.contains(&GizmoModeKind::Translate) {
            return Err(refuse("position"));
        }
        transform.translation = position(data, p.cell, p.translation, 0.0)?;
    }
    if let Some([x, y, z]) = p.rotation_deg {
        if !allowed.contains(&GizmoModeKind::Rotate) || kind == NodeKind::Sun {
            return Err(refuse(
                "rotation (the sun uses light.elevation_deg/azimuth_deg)",
            ));
        }
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            x.to_radians(),
            y.to_radians(),
            z.to_radians(),
        );
    }
    if let Some(scale) = p.scale {
        if !allowed.contains(&GizmoModeKind::Scale) {
            return Err(refuse("scale"));
        }
        transform.scale = Vec3::from_array(scale);
    }
    if let Some(light) = sun_angles {
        if kind != NodeKind::Sun {
            return Err(AgentError::Invalid(
                "elevation_deg and azimuth_deg are sun fields".into(),
            ));
        }
        if let Some(degrees) = light.elevation_deg {
            if !(0.0..=90.0).contains(&degrees) {
                return Err(AgentError::Invalid("elevation_deg must be 0..=90".into()));
            }
            transform.rotation = with_sun_elevation(transform.rotation, degrees.to_radians());
        }
        if let Some(degrees) = light.azimuth_deg {
            let (_, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
            transform.rotation = Quat::from_euler(EulerRot::YXZ, degrees.to_radians(), pitch, roll);
        }
    }
    Ok(Some(transform))
}

/// Prop, sound and effect field edits; each needs the matching node kind.
fn payload_edits(
    world: &World,
    id: EditorNodeId,
    node: Entity,
    kind: NodeKind,
    p: &UpdateNodeParams,
) -> Result<Vec<Box<dyn EditOp>>, AgentError> {
    let mut edits: Vec<Box<dyn EditOp>> = Vec::new();
    let wrong = |field: &str| {
        AgentError::Invalid(format!(
            "node {} is a {}, not a {field}",
            id.0,
            kind_name(kind)
        ))
    };
    if let Some(prop) = &p.prop {
        if kind != NodeKind::Prop {
            return Err(wrong("prop"));
        }
        if let Some(model) = &prop.model {
            require_file(world, model)?;
        }
        edits.push(edit(world, id, node, "prop", |r: &mut LifPropRef| {
            if let Some(model) = &prop.model {
                r.0.model = model.clone();
            }
            if let Some(anim_type) = prop.anim_type {
                r.0.anim_type = anim_type;
            }
            if let Some(anim_speed) = prop.anim_speed {
                r.0.anim_speed = anim_speed;
            }
        })?);
    }
    if let Some(sound) = &p.sound {
        if kind != NodeKind::Sound {
            return Err(wrong("sound"));
        }
        if let Some(file) = &sound.file {
            require_file(world, file)?;
        }
        edits.push(edit(
            world,
            id,
            node,
            "sound",
            |s: &mut LifAudioEmitter| {
                if let Some(file) = &sound.file {
                    s.0.file = file.clone();
                }
                if let Some(volume) = sound.volume {
                    s.0.volume = volume;
                }
                if let Some(range) = sound.range {
                    s.0.range = range;
                }
                if let Some(cycle) = sound.cycle {
                    s.0.cycle = cycle;
                }
            },
        )?);
    }
    if let Some(effect) = &p.effect {
        if kind != NodeKind::Effect {
            return Err(wrong("effect"));
        }
        if let Some(effect_type) = effect.effect_type {
            require_effect(world, effect_type)?;
        }
        edits.push(edit(
            world,
            id,
            node,
            "effect",
            |e: &mut LifEffectEmitter| {
                if let Some(effect_type) = effect.effect_type {
                    e.0.effect_type = effect_type;
                }
                if let Some(emit_speed) = effect.emit_speed {
                    e.0.emit_speed = emit_speed;
                }
                if let Some(params) = effect.params {
                    e.0.params = params;
                }
            },
        )?);
    }
    Ok(edits)
}

/// Light field edits for the node's kind; sun angles are part of its transform.
fn light_edits(
    world: &World,
    id: EditorNodeId,
    node: Entity,
    kind: NodeKind,
    light: &LightPatch,
) -> Result<Vec<Box<dyn EditOp>>, AgentError> {
    let mut edits: Vec<Box<dyn EditOp>> = Vec::new();
    let color = light.color.map(|[r, g, b]| Color::srgb(r, g, b));
    let not_for = |field: &str| {
        AgentError::Invalid(format!("`{field}` does not apply to a {}", kind_name(kind)))
    };
    if light.lux.is_some() && kind != NodeKind::Sun {
        return Err(not_for("lux"));
    }
    match kind {
        NodeKind::Sun => {
            if light.candela.is_some() {
                return Err(not_for("candela"));
            }
            if light.range.is_some() {
                return Err(not_for("range"));
            }
            if light.inner_angle_deg.is_some() || light.outer_angle_deg.is_some() {
                return Err(not_for("cone angles"));
            }
            if color.is_some() || light.lux.is_some() {
                edits.push(edit(
                    world,
                    id,
                    node,
                    "light",
                    |sun: &mut DirectionalLight| {
                        if let Some(color) = color {
                            sun.color = color;
                        }
                        if let Some(lux) = light.lux {
                            sun.illuminance = lux;
                        }
                    },
                )?);
            }
        }
        NodeKind::PointLight => {
            if light.inner_angle_deg.is_some() || light.outer_angle_deg.is_some() {
                return Err(not_for("cone angles"));
            }
            if color.is_some() || light.range.is_some() {
                edits.push(edit(world, id, node, "light", |point: &mut PointLight| {
                    if let Some(color) = color {
                        point.color = color;
                    }
                    if let Some(range) = light.range {
                        point.range = range;
                    }
                })?);
            }
        }
        NodeKind::SpotLight => {
            let holder = component_holder::<SpotLight>(world, node);
            let current = world
                .get::<SpotLight>(holder)
                .ok_or_else(|| AgentError::NotFound("the spot light".into()))?;
            let inner = light
                .inner_angle_deg
                .map_or(current.inner_angle, f32::to_radians);
            let outer = light
                .outer_angle_deg
                .map_or(current.outer_angle, f32::to_radians);
            if !spot_angles_valid(inner, outer) {
                return Err(AgentError::Invalid(
                    "spot angles need 0 < inner <= outer <= 90 degrees".into(),
                ));
            }
            let changes = color.is_some()
                || light.range.is_some()
                || light.inner_angle_deg.is_some()
                || light.outer_angle_deg.is_some();
            if changes {
                edits.push(edit(world, id, node, "light", |spot: &mut SpotLight| {
                    if let Some(color) = color {
                        spot.color = color;
                    }
                    if let Some(range) = light.range {
                        spot.range = range;
                    }
                    spot.inner_angle = inner;
                    spot.outer_angle = outer;
                })?);
            }
        }
        NodeKind::Prop | NodeKind::Sound | NodeKind::Effect => {
            return Err(AgentError::Invalid(format!(
                "node {} is a {}, not a light",
                id.0,
                kind_name(kind)
            )));
        }
    }
    if let Some(candela) = light.candela {
        edits.push(edit(
            world,
            id,
            node,
            "candela",
            |c: &mut MapLightCandela| {
                c.0 = candela;
            },
        )?);
    }
    Ok(edits)
}
