//! Editor handles make lights and emitters selectable without reaching the
//! saved glb; inspector edits refresh what the runtime spawned; delete is
//! undoable and refused for the sun.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        layout::MapLayout,
        refresh::after_node_edit,
        save::{SaveRequest, map_glb_path},
    },
    shell::{
        node::EditorNodeId,
        selection::{
            EditorHandle, Selection, all_nodes, delete_selected, invert_selection, object_node,
            select_all,
        },
        undo::{ComponentEdit, UndoStack, commit, undo},
    },
};
use lifthrasir_runtime::{
    audio::map_sounds::MapSoundSource,
    world::{EmitterOutput, LifAudioEmitter, LifEffectEmitter, LifPropRef},
};

fn object_nodes(app: &App, layout: &MapLayout) -> Vec<Entity> {
    app.world()
        .get::<Children>(layout.map_node)
        .unwrap()
        .iter()
        .filter(|&child| Some(child) != layout.terrain_node)
        .collect()
}

/// The object node with a child carrying `T` (lights live on a child).
fn node_with_child<T: Component>(app: &App, layout: &MapLayout) -> Entity {
    let world = app.world();
    object_nodes(app, layout)
        .into_iter()
        .find(|&node| {
            world
                .get::<Children>(node)
                .is_some_and(|children| children.iter().any(|c| world.get::<T>(c).is_some()))
        })
        .unwrap()
}

fn node_with<T: Component>(app: &App, layout: &MapLayout) -> Entity {
    object_nodes(app, layout)
        .into_iter()
        .find(|&node| app.world().get::<T>(node).is_some())
        .unwrap()
}

fn handles(app: &App, node: Entity) -> Vec<Entity> {
    app.world()
        .get::<Children>(node)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&child| app.world().get::<EditorHandle>(child).is_some())
        .collect()
}

#[test]
fn lights_and_emitters_get_one_handle_that_never_reaches_the_glb() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    app.update();

    let handled = [
        node_with::<LifAudioEmitter>(&app, &layout),
        node_with::<LifEffectEmitter>(&app, &layout),
        node_with_child::<PointLight>(&app, &layout),
        node_with_child::<SpotLight>(&app, &layout),
    ];
    for node in handled {
        let found = handles(&app, node);
        assert_eq!(found.len(), 1, "{node:?} should have exactly one handle");
        let parent_of = |entity| app.world().get::<ChildOf>(entity).map(ChildOf::parent);
        assert_eq!(object_node(found[0], parent_of, &layout), Some(node));
    }
    assert!(handles(&app, node_with::<LifPropRef>(&app, &layout)).is_empty());
    assert!(handles(&app, node_with_child::<DirectionalLight>(&app, &layout)).is_empty());

    app.world_mut().write_message(SaveRequest { save_as: None });
    app.update();
    let saved = std::fs::read(map_glb_path(data_folder.path(), &layout.name)).unwrap();
    let fixture = std::fs::read(common::fixtures_dir().join("mini_map.glb")).unwrap();
    assert!(saved == fixture, "handles leaked into the saved glb");
}

#[test]
fn the_terrain_is_never_an_object_node() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let terrain = layout.terrain_node.unwrap();
    let primitive = app.world().get::<Children>(terrain).unwrap()[0];

    let parent_of = |entity| app.world().get::<ChildOf>(entity).map(ChildOf::parent);

    assert_eq!(object_node(primitive, parent_of, &layout), None);
}

#[test]
fn a_sound_edit_respawns_its_output_with_the_new_volume() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    app.update();
    let node = node_with::<LifAudioEmitter>(&app, &layout);
    let id = *app.world().get::<EditorNodeId>(node).unwrap();
    let before = app.world().get::<LifAudioEmitter>(node).unwrap().clone();
    let mut after = before.clone();
    after.0.volume = before.0.volume * 0.5 + 0.1;

    commit(
        app.world_mut(),
        Box::new(ComponentEdit::new(id, before, after.clone()).with_hook(after_node_edit)),
    );
    app.update();
    app.update();

    let volumes: Vec<f32> = app
        .world_mut()
        .query::<(&EmitterOutput, &MapSoundSource)>()
        .iter(app.world())
        .filter(|(output, _)| output.0 == node)
        .map(|(_, source)| source.base_volume)
        .collect();
    assert_eq!(volumes, [after.0.volume]);
    assert_eq!(
        app.world().get::<LifAudioEmitter>(node).unwrap().0.volume,
        after.0.volume
    );
}

#[test]
fn deleting_a_prop_is_undoable_and_the_sun_cannot_be_deleted() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let prop = node_with::<LifPropRef>(&app, &layout);
    let prop_id = *app.world().get::<EditorNodeId>(prop).unwrap();

    app.world_mut()
        .resource_mut::<Selection>()
        .replace([prop_id]);
    delete_selected(app.world_mut());

    assert!(app.world().get_entity(prop).is_err());
    assert!(app.world().resource::<Selection>().is_empty());
    undo(app.world_mut());
    let restored = app
        .world_mut()
        .query::<(Entity, &EditorNodeId)>()
        .iter(app.world())
        .find_map(|(entity, id)| (*id == prop_id).then_some(entity))
        .unwrap();
    assert!(app.world().get::<LifPropRef>(restored).is_some());

    let sun = node_with_child::<DirectionalLight>(&app, &layout);
    let sun_id = *app.world().get::<EditorNodeId>(sun).unwrap();
    app.world_mut().resource_mut::<UndoStack>().clear();
    app.world_mut()
        .resource_mut::<Selection>()
        .replace([sun_id]);
    delete_selected(app.world_mut());

    assert!(app.world().get_entity(sun).is_ok());
    assert!(app.world().resource::<UndoStack>().is_pristine());
}

#[test]
fn select_all_invert_and_multi_delete_are_one_undo_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    app.update();
    let all = all_nodes(app.world());
    assert!(all.len() > 2);

    select_all(app.world_mut());
    assert_eq!(app.world().resource::<Selection>().nodes(), all);
    invert_selection(app.world_mut());
    assert!(app.world().resource::<Selection>().is_empty());
    invert_selection(app.world_mut());
    assert_eq!(app.world().resource::<Selection>().nodes(), all);

    app.world_mut().resource_mut::<UndoStack>().clear();
    delete_selected(app.world_mut());
    let left = all_nodes(app.world());
    assert_eq!(left.len(), 1, "only the sun survives");
    assert_eq!(app.world().resource::<Selection>().nodes(), left);

    undo(app.world_mut());
    assert_eq!(all_nodes(app.world()).len(), all.len());
}
