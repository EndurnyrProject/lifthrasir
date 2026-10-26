//! Undo history. Every user gesture commits exactly one [`EditOp`]; ops address
//! map nodes by [`EditorNodeId`] so they survive delete then undo.

use super::{
    node::{EditorNodeId, NodeSnapshot, find_node, spawn_node},
    ui::command_modifier,
};
use crate::map::terrain::{MeshSnapshot, sync_chunk_visibility};
use bevy::{prelude::*, utils::prelude::ShortName};
use lifthrasir_runtime::world::LifMapData;

/// One undoable edit. `apply` and `undo` must be exact inverses.
pub trait EditOp: Send + Sync + 'static {
    fn apply(&mut self, world: &mut World);
    fn undo(&mut self, world: &mut World);
    fn label(&self) -> &str;
}

/// Runs after an op changed something the runtime does not observe by itself.
/// Receives the node entity (or the map root for [`MapDataEdit`]).
pub type EditHook = fn(&mut World, Entity);

/// Replaces component `T` on a node, or on its first child carrying `T`
/// (lights live on a child of their node).
pub struct ComponentEdit<T: Component + Clone> {
    node: EditorNodeId,
    before: T,
    after: T,
    hook: Option<EditHook>,
    label: String,
}

impl<T: Component + Clone> ComponentEdit<T> {
    pub fn new(node: EditorNodeId, before: T, after: T) -> Self {
        Self {
            node,
            before,
            after,
            hook: None,
            label: format!("Edit {}", ShortName::of::<T>()),
        }
    }

    pub fn with_hook(mut self, hook: EditHook) -> Self {
        self.hook = Some(hook);
        self
    }

    fn write(&self, world: &mut World, value: T) {
        let Some(node) = find_node(world, self.node) else {
            error!("{}: node {:?} no longer exists", self.label, self.node);
            return;
        };
        let target = component_holder::<T>(world, node);
        world.entity_mut(target).insert(value);
        if let Some(hook) = self.hook {
            hook(world, node);
        }
    }
}

/// `node` itself when it has `T`, otherwise its first child with `T`, otherwise
/// `node` (the edit then adds `T` there).
pub(crate) fn component_holder<T: Component>(world: &World, node: Entity) -> Entity {
    if world.get::<T>(node).is_some() {
        return node;
    }
    world
        .get::<Children>(node)
        .and_then(|children| {
            children
                .iter()
                .find(|&child| world.get::<T>(child).is_some())
        })
        .unwrap_or(node)
}

impl<T: Component + Clone> EditOp for ComponentEdit<T> {
    fn apply(&mut self, world: &mut World) {
        self.write(world, self.after.clone());
    }

    fn undo(&mut self, world: &mut World) {
        self.write(world, self.before.clone());
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// Adds an object node under `parent`.
pub struct SpawnNode {
    root: Entity,
    parent: Entity,
    snapshot: NodeSnapshot,
    index: Option<usize>,
    label: String,
}

impl SpawnNode {
    pub fn new(root: Entity, parent: Entity, snapshot: NodeSnapshot, index: Option<usize>) -> Self {
        let label = format!("Add {}", snapshot.name);
        Self {
            root,
            parent,
            snapshot,
            index,
            label,
        }
    }
}

impl EditOp for SpawnNode {
    fn apply(&mut self, world: &mut World) {
        spawn_node(world, self.root, self.parent, &self.snapshot, self.index);
    }

    fn undo(&mut self, world: &mut World) {
        despawn_by_id(world, self.snapshot.id, &self.label);
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// Removes an object node; undo respawns it with the same id at the same
/// child index.
pub struct DespawnNode {
    root: Entity,
    parent: Entity,
    node: EditorNodeId,
    snapshot: Option<NodeSnapshot>,
    index: Option<usize>,
}

impl DespawnNode {
    pub fn new(root: Entity, parent: Entity, node: EditorNodeId) -> Self {
        Self {
            root,
            parent,
            node,
            snapshot: None,
            index: None,
        }
    }
}

impl EditOp for DespawnNode {
    fn apply(&mut self, world: &mut World) {
        let Some(entity) = find_node(world, self.node) else {
            error!("Delete: node {:?} no longer exists", self.node);
            return;
        };
        let Some(snapshot) = NodeSnapshot::capture(world, entity) else {
            error!("Delete: node {:?} cannot be restored, refusing", self.node);
            return;
        };
        self.index = world
            .get::<Children>(self.parent)
            .and_then(|children| children.iter().position(|child| child == entity));
        self.snapshot = Some(snapshot);
        world.entity_mut(entity).despawn();
    }

    fn undo(&mut self, world: &mut World) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        spawn_node(world, self.root, self.parent, snapshot, self.index);
    }

    fn label(&self) -> &str {
        "Delete"
    }
}

fn despawn_by_id(world: &mut World, id: EditorNodeId, label: &str) {
    let Some(entity) = find_node(world, id) else {
        error!("{label}: node {id:?} no longer exists");
        return;
    };
    world.entity_mut(entity).despawn();
}

/// Replaces the map root's authoritative [`LifMapData`].
pub struct MapDataEdit {
    root: Entity,
    before: LifMapData,
    after: LifMapData,
    hook: Option<EditHook>,
}

impl MapDataEdit {
    pub fn new(root: Entity, before: LifMapData, after: LifMapData) -> Self {
        Self {
            root,
            before,
            after,
            hook: None,
        }
    }

    pub fn with_hook(mut self, hook: EditHook) -> Self {
        self.hook = Some(hook);
        self
    }

    fn write(&self, world: &mut World, data: LifMapData) {
        world.entity_mut(self.root).insert(data);
        if let Some(hook) = self.hook {
            hook(world, self.root);
        }
    }
}

impl EditOp for MapDataEdit {
    fn apply(&mut self, world: &mut World) {
        self.write(world, self.after.clone());
    }

    fn undo(&mut self, world: &mut World) {
        self.write(world, self.before.clone());
    }

    fn label(&self) -> &str {
        "Edit map data"
    }
}

/// Replaces one terrain chunk mesh. A chunk left empty is hidden, any other
/// shown; chunk entities are never despawned while the map is open.
pub struct MeshChunkEdit {
    chunk: Entity,
    mesh: Handle<Mesh>,
    before: MeshSnapshot,
    after: MeshSnapshot,
}

impl MeshChunkEdit {
    pub fn new(
        chunk: Entity,
        mesh: Handle<Mesh>,
        before: MeshSnapshot,
        after: MeshSnapshot,
    ) -> Self {
        Self {
            chunk,
            mesh,
            before,
            after,
        }
    }

    fn write(&self, world: &mut World, snapshot: &MeshSnapshot) {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let Some(mut mesh) = meshes.get_mut(&self.mesh) else {
            error!("Edit terrain: chunk {:?} mesh is gone", self.chunk);
            return;
        };
        snapshot.restore(&mut mesh);
        drop(mesh);
        sync_chunk_visibility(world, self.chunk, snapshot.is_empty());
    }
}

impl EditOp for MeshChunkEdit {
    fn apply(&mut self, world: &mut World) {
        let after = std::mem::take(&mut self.after);
        self.write(world, &after);
        self.after = after;
    }

    fn undo(&mut self, world: &mut World) {
        let before = std::mem::take(&mut self.before);
        self.write(world, &before);
        self.before = before;
    }

    fn label(&self) -> &str {
        "Edit terrain"
    }
}

/// Several ops as one gesture: applied in order, undone in reverse.
pub struct Batch(pub Vec<Box<dyn EditOp>>);

impl EditOp for Batch {
    fn apply(&mut self, world: &mut World) {
        for op in &mut self.0 {
            op.apply(world);
        }
    }

    fn undo(&mut self, world: &mut World) {
        for op in self.0.iter_mut().rev() {
            op.undo(world);
        }
    }

    fn label(&self) -> &str {
        self.0.first().map_or("Batch", |op| op.label())
    }
}

/// Done and undone ops plus the depth of the last save.
#[derive(Resource)]
pub struct UndoStack {
    done: Vec<Box<dyn EditOp>>,
    undone: Vec<Box<dyn EditOp>>,
    /// `done.len()` at the last save; `None` once that state is unreachable.
    saved_at: Option<usize>,
    /// Bumped by every push, undo and redo, so watchers can tell that the
    /// document changed even when `done.len()` did not.
    revision: u64,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            saved_at: Some(0),
            revision: 0,
        }
    }
}

impl UndoStack {
    /// True when the world differs from the last save (or the open).
    pub fn is_dirty(&self) -> bool {
        self.saved_at != Some(self.done.len())
    }

    /// True when nothing has been edited since the open.
    pub fn is_pristine(&self) -> bool {
        self.done.is_empty()
    }

    /// Changes whenever the document does; not comparable across `clear`.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn mark_saved(&mut self) {
        self.saved_at = Some(self.done.len());
    }

    /// Forgets all history; the current state counts as saved.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.done.last().map(|op| op.label())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.undone.last().map(|op| op.label())
    }

    fn push(&mut self, op: Box<dyn EditOp>) {
        if self.saved_at.is_some_and(|saved| saved > self.done.len()) {
            self.saved_at = None;
        }
        self.done.push(op);
        self.undone.clear();
        self.revision += 1;
    }
}

/// Applies `op` and pushes it; clears redo.
pub fn commit(world: &mut World, mut op: Box<dyn EditOp>) {
    op.apply(world);
    world.resource_mut::<UndoStack>().push(op);
}

/// Pushes an op whose effect is already live (drags, strokes); clears redo.
pub fn record(world: &mut World, op: Box<dyn EditOp>) {
    world.resource_mut::<UndoStack>().push(op);
}

pub fn undo(world: &mut World) {
    let Some(mut op) = world.resource_mut::<UndoStack>().done.pop() else {
        return;
    };
    op.undo(world);
    let mut stack = world.resource_mut::<UndoStack>();
    stack.undone.push(op);
    stack.revision += 1;
}

pub fn redo(world: &mut World) {
    let Some(mut op) = world.resource_mut::<UndoStack>().undone.pop() else {
        return;
    };
    op.apply(world);
    let mut stack = world.resource_mut::<UndoStack>();
    stack.done.push(op);
    stack.revision += 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryKey {
    Undo,
    Redo,
}

/// Ctrl/Cmd+Z undoes; Ctrl/Cmd+Y and Ctrl/Cmd+Shift+Z redo.
fn history_key(keys: &ButtonInput<KeyCode>) -> Option<HistoryKey> {
    if !command_modifier(keys) {
        return None;
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if keys.just_pressed(KeyCode::KeyY) || (shift && keys.just_pressed(KeyCode::KeyZ)) {
        return Some(HistoryKey::Redo);
    }
    keys.just_pressed(KeyCode::KeyZ).then_some(HistoryKey::Undo)
}

/// Undo/redo shortcuts; run only while the viewport owns the keyboard.
pub fn history_keys(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    match history_key(&keys) {
        Some(HistoryKey::Undo) => commands.queue(undo),
        Some(HistoryKey::Redo) => commands.queue(redo),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::node::{EditorNodeIds, NodePayload};
    use lifthrasir_data::lif::{LifMap, LifProp};
    use lifthrasir_runtime::world::{LifPropRef, PropSceneContext, SceneOwner};
    use ro_formats::RoAltitude;

    struct Scene {
        world: World,
        root: Entity,
        map_node: Entity,
    }

    fn scene() -> Scene {
        let mut world = World::new();
        world.init_resource::<UndoStack>();
        world.init_resource::<EditorNodeIds>();
        let root = world
            .spawn(PropSceneContext {
                no_shade_tint: [0.5, 0.6, 0.7],
            })
            .id();
        let map_node = world.spawn((Name::new("map"), ChildOf(root))).id();
        Scene {
            world,
            root,
            map_node,
        }
    }

    fn prop_snapshot(world: &mut World, name: &str) -> NodeSnapshot {
        NodeSnapshot {
            id: world.resource_mut::<EditorNodeIds>().allocate(),
            name: name.into(),
            transform: Transform::from_xyz(1.0, 0.0, 0.0),
            payload: NodePayload::Prop(LifProp {
                model: format!("ro://models/{name}.glb"),
                anim_type: 0,
                anim_speed: 1.0,
            }),
        }
    }

    fn spawn_prop(scene: &mut Scene, name: &str) -> (EditorNodeId, Entity) {
        let snapshot = prop_snapshot(&mut scene.world, name);
        let entity = spawn_node(
            &mut scene.world,
            scene.root,
            scene.map_node,
            &snapshot,
            None,
        );
        (snapshot.id, entity)
    }

    fn translation(world: &mut World, id: EditorNodeId) -> Vec3 {
        let entity = find_node(world, id).unwrap();
        world.get::<Transform>(entity).unwrap().translation
    }

    fn move_op(id: EditorNodeId, x: f32) -> Box<dyn EditOp> {
        Box::new(ComponentEdit::new(
            id,
            Transform::from_xyz(1.0, 0.0, 0.0),
            Transform::from_xyz(x, 0.0, 0.0),
        ))
    }

    fn map_data(width: u32) -> LifMapData {
        LifMapData {
            altitude: RoAltitude {
                version: "1.2".into(),
                width,
                height: 1,
                cells: Vec::new(),
            },
            water: None,
            water_tiles: Vec::new(),
            meta: LifMap {
                format_version: 0,
                source: None,
                ambient_color: [0.0; 3],
                no_shade_tint: [1.0; 3],
                indoor: false,
                ambient_brightness: 0.0,
                exposure_ev100: 0.0,
            },
        }
    }

    #[test]
    fn component_edit_applies_undoes_and_redoes() {
        let mut scene = scene();
        let (id, _) = spawn_prop(&mut scene, "tree");

        commit(&mut scene.world, move_op(id, 5.0));
        assert_eq!(translation(&mut scene.world, id).x, 5.0);
        undo(&mut scene.world);
        assert_eq!(translation(&mut scene.world, id).x, 1.0);
        redo(&mut scene.world);
        assert_eq!(translation(&mut scene.world, id).x, 5.0);
    }

    #[test]
    fn component_edit_writes_a_light_on_the_node_child_and_runs_its_hook() {
        #[derive(Resource)]
        struct Hooked(Entity);
        fn hook(world: &mut World, node: Entity) {
            world.insert_resource(Hooked(node));
        }
        let mut scene = scene();
        let snapshot = NodeSnapshot {
            id: EditorNodeId(99),
            name: "lamp".into(),
            transform: Transform::IDENTITY,
            payload: NodePayload::PointLight {
                light: PointLight::default(),
                candela: 10.0,
            },
        };
        let node = spawn_node(
            &mut scene.world,
            scene.root,
            scene.map_node,
            &snapshot,
            None,
        );
        let light = crate::shell::node::light_entity(&scene.world, node).unwrap();
        let before = *scene.world.get::<PointLight>(light).unwrap();
        let after = PointLight {
            range: 77.0,
            ..before
        };

        commit(
            &mut scene.world,
            Box::new(ComponentEdit::new(EditorNodeId(99), before, after).with_hook(hook)),
        );

        assert_eq!(scene.world.get::<PointLight>(light).unwrap().range, 77.0);
        assert!(scene.world.get::<PointLight>(node).is_none());
        assert_eq!(scene.world.resource::<Hooked>().0, node);
    }

    #[test]
    fn spawn_node_op_undoes_by_id() {
        let mut scene = scene();
        let snapshot = prop_snapshot(&mut scene.world, "rock");
        let id = snapshot.id;

        commit(
            &mut scene.world,
            Box::new(SpawnNode::new(scene.root, scene.map_node, snapshot, None)),
        );
        assert!(find_node(&mut scene.world, id).is_some());
        undo(&mut scene.world);
        assert!(find_node(&mut scene.world, id).is_none());
        redo(&mut scene.world);
        assert!(find_node(&mut scene.world, id).is_some());
    }

    #[test]
    fn despawned_prop_comes_back_with_its_id_index_and_context() {
        let mut scene = scene();
        let (first, _) = spawn_prop(&mut scene, "first");
        let (middle, _) = spawn_prop(&mut scene, "middle");
        spawn_prop(&mut scene, "last");
        commit(&mut scene.world, move_op(middle, 9.0));

        commit(
            &mut scene.world,
            Box::new(DespawnNode::new(scene.root, scene.map_node, middle)),
        );
        assert!(find_node(&mut scene.world, middle).is_none());
        undo(&mut scene.world);

        let entity = find_node(&mut scene.world, middle).unwrap();
        let children = scene.world.get::<Children>(scene.map_node).unwrap();
        assert_eq!(children.iter().position(|child| child == entity), Some(1));
        assert!(scene.world.get::<LifPropRef>(entity).is_some());
        assert_eq!(
            scene.world.get::<SceneOwner>(entity),
            Some(&SceneOwner(scene.root))
        );
        assert_eq!(
            scene.world.get::<PropSceneContext>(entity),
            Some(&PropSceneContext {
                no_shade_tint: [0.5, 0.6, 0.7]
            })
        );
        assert_eq!(translation(&mut scene.world, middle).x, 9.0);
        undo(&mut scene.world);
        assert_eq!(translation(&mut scene.world, middle).x, 1.0);
        assert_eq!(translation(&mut scene.world, first).x, 1.0);
    }

    #[test]
    fn map_data_edit_swaps_the_root_copy_and_runs_its_hook_on_the_root() {
        #[derive(Resource)]
        struct Hooked(Entity);
        fn hook(world: &mut World, root: Entity) {
            world.insert_resource(Hooked(root));
        }
        let mut scene = scene();
        scene.world.entity_mut(scene.root).insert(map_data(1));
        let width = |world: &World, root| world.get::<LifMapData>(root).unwrap().altitude.width;

        commit(
            &mut scene.world,
            Box::new(MapDataEdit::new(scene.root, map_data(1), map_data(2)).with_hook(hook)),
        );
        assert_eq!(width(&scene.world, scene.root), 2);
        assert_eq!(scene.world.resource::<Hooked>().0, scene.root);
        undo(&mut scene.world);
        assert_eq!(width(&scene.world, scene.root), 1);
    }

    #[test]
    fn batch_undoes_in_reverse_order() {
        let mut scene = scene();
        let (id, _) = spawn_prop(&mut scene, "tree");
        let batch = Batch(vec![
            move_op(id, 5.0),
            Box::new(ComponentEdit::new(
                id,
                Transform::from_xyz(5.0, 0.0, 0.0),
                Transform::from_xyz(8.0, 0.0, 0.0),
            )),
        ]);

        commit(&mut scene.world, Box::new(batch));
        assert_eq!(translation(&mut scene.world, id).x, 8.0);
        undo(&mut scene.world);
        assert_eq!(translation(&mut scene.world, id).x, 1.0);
    }

    #[test]
    fn a_new_commit_clears_redo() {
        let mut scene = scene();
        let (id, _) = spawn_prop(&mut scene, "tree");
        commit(&mut scene.world, move_op(id, 5.0));
        undo(&mut scene.world);

        commit(&mut scene.world, move_op(id, 6.0));

        assert_eq!(scene.world.resource::<UndoStack>().redo_label(), None);
    }

    #[test]
    fn dirty_tracks_the_save_point() {
        let mut scene = scene();
        let (id, _) = spawn_prop(&mut scene, "tree");
        let stack = |world: &World| {
            let stack = world.resource::<UndoStack>();
            (stack.is_dirty(), stack.is_pristine())
        };
        assert_eq!(stack(&scene.world), (false, true));

        commit(&mut scene.world, move_op(id, 5.0));
        assert_eq!(stack(&scene.world), (true, false));
        scene.world.resource_mut::<UndoStack>().mark_saved();
        assert_eq!(stack(&scene.world), (false, false));
        undo(&mut scene.world);
        assert_eq!(stack(&scene.world), (true, true));
        redo(&mut scene.world);
        assert_eq!(stack(&scene.world), (false, false));
        undo(&mut scene.world);
        commit(&mut scene.world, move_op(id, 6.0));
        assert_eq!(stack(&scene.world), (true, false));
        undo(&mut scene.world);
        assert!(scene.world.resource::<UndoStack>().is_dirty());
    }

    #[test]
    fn record_pushes_without_applying() {
        let mut scene = scene();
        let (id, _) = spawn_prop(&mut scene, "tree");

        record(&mut scene.world, move_op(id, 5.0));

        assert_eq!(translation(&mut scene.world, id).x, 1.0);
        assert_eq!(
            scene.world.resource::<UndoStack>().undo_label(),
            Some("Edit Transform")
        );
    }

    #[test]
    fn shortcuts_map_to_undo_and_redo() {
        let press = |keys: &[KeyCode]| {
            let mut input = ButtonInput::<KeyCode>::default();
            for key in keys {
                input.press(*key);
            }
            history_key(&input)
        };
        assert_eq!(
            press(&[KeyCode::ControlLeft, KeyCode::KeyZ]),
            Some(HistoryKey::Undo)
        );
        assert_eq!(
            press(&[KeyCode::SuperLeft, KeyCode::KeyZ]),
            Some(HistoryKey::Undo)
        );
        assert_eq!(
            press(&[KeyCode::ControlLeft, KeyCode::KeyY]),
            Some(HistoryKey::Redo)
        );
        assert_eq!(
            press(&[KeyCode::SuperRight, KeyCode::ShiftLeft, KeyCode::KeyZ]),
            Some(HistoryKey::Redo)
        );
        assert_eq!(press(&[KeyCode::KeyZ]), None);
    }
}
