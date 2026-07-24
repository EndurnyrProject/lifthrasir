# glTF (Bevy 0.19)

Reference distilled from `bevy/examples/gltf/`. Verified 0.19 facts: scenes spawn via `WorldAssetRoot` (was `SceneRoot`), spawn-complete signal is `WorldInstanceReady` (was `SceneInstanceReady`, now in `bevy::world_serialization`), no bundles, observers use `On<...>`.

## Loading a scene: `GltfAssetLabel` + `WorldAssetRoot`

A glTF file is a container asset; sub-assets are addressed with typed labels. `GltfAssetLabel::X.from_asset(path)` builds the labeled `AssetPath` (equivalent to string form `"file.gltf#Scene0"`). Spawning an entity with `WorldAssetRoot(Handle<WorldAsset>)` instantiates the scene as children of that entity; sibling `Transform` places the whole instance.

```rust
commands.spawn(WorldAssetRoot(asset_server.load(
    GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf"),
)));
```

`GltfAssetLabel` variants (from `bevy_gltf/src/label.rs`) and what they load:

| Variant | String label | Bevy asset type |
|---|---|---|
| `Scene(usize)` | `Scene0` | `WorldAsset` |
| `Node(usize)` | `Node0` | `GltfNode` |
| `Mesh(usize)` | `Mesh0` | `GltfMesh` |
| `Primitive { mesh, primitive }` | `Mesh0/Primitive0` | `Mesh` |
| `Texture(usize)` | `Texture0` | `Image` |
| `Material { index, is_scale_inverted }` | `Material0` | `GltfMaterial` |
| `DefaultMaterial` | `DefaultMaterial` | material |
| `Animation(usize)` | `Animation0` | `AnimationClip` |
| `Skin(usize)` | `Skin0` | `GltfSkin` |
| `InverseBindMatrices(usize)` | `Skin0/InverseBindMatrices` | `SkinnedMeshInverseBindposes` |

Source: `examples/gltf/load_gltf.rs`

## Loading with settings (`GltfLoaderSettings`)

Use the load builder to tweak the loader per-asset. Fields: `load_meshes: RenderAssetUsages`, `load_materials: RenderAssetUsages`, `load_cameras: bool`, `load_lights: bool`, `include_source: bool`, `convert_coordinates: Option<GltfConvertCoordinates>`. Key gotcha: to mutate a loaded `Mesh` via `Assets<Mesh>` at runtime, `load_meshes` must include `MAIN_WORLD` (`RenderAssetUsages::all()`); `RENDER_WORLD`-only is cheaper but CPU-side data is dropped.

```rust
use bevy::{asset::RenderAssetUsages, gltf::GltfLoaderSettings};

let mesh: Handle<Mesh> = asset_server
    .load_builder()
    .with_settings(|s: &mut GltfLoaderSettings| s.load_meshes = RenderAssetUsages::all())
    .load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset("models/cube/cube.gltf"));
```

Source: `examples/asset/alter_mesh.rs`, `examples/testbed/3d.rs` (coordinate conversion), `crates/bevy_gltf/src/loader/mod.rs`

## Custom vertex attributes from glTF

Map a glTF attribute name to a Bevy `MeshVertexAttribute` via `GltfPlugin::add_custom_vertex_attribute` when configuring `DefaultPlugins`. Note the underscore quirk: a file attribute named `__BARYCENTRIC` is registered as `"_BARYCENTRIC"` (one leading underscore stripped for comparison). The custom material's `specialize` then binds the attribute into the vertex layout. Directly relevant as the reference design for an RO→glTF converter carrying RO-specific per-vertex data.

```rust
const ATTRIBUTE_BARYCENTRIC: MeshVertexAttribute =
    MeshVertexAttribute::new("Barycentric", 2137464976, VertexFormat::Float32x3);

App::new().add_plugins((
    DefaultPlugins.set(
        GltfPlugin::default().add_custom_vertex_attribute("_BARYCENTRIC", ATTRIBUTE_BARYCENTRIC),
    ),
    Material2dPlugin::<CustomMaterial>::default(),
));

// in Material2d::specialize:
let vertex_layout = layout.0.get_layout(&[
    Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
    Mesh::ATTRIBUTE_COLOR.at_shader_location(1),
    ATTRIBUTE_BARYCENTRIC.at_shader_location(2),
])?;
descriptor.vertex.buffers = vec![vertex_layout];
```

Source: `examples/gltf/custom_gltf_vertex_attribute.rs`

## Editing materials after spawn (`WorldInstanceReady` + `GltfMaterialName`)

Scene instances spawn asynchronously; react with an observer on `WorldInstanceReady` (global `add_observer` or per-entity `.observe`). `scene_ready.entity` is the `WorldAssetRoot` entity. Walk descendants with `Children::iter_descendants`, identify targets by the `GltfMaterialName` component the loader adds, then clone-modify the material and insert a new `MeshMaterial3d`. Cache the new handle if reused — duplicate identical materials are expensive.

```rust
app.add_observer(change_material);

fn change_material(
    scene_ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    color_override: Query<&ColorOverride>,
    mesh_materials: Query<(&MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    mut asset_materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(color_override) = color_override.get(scene_ready.entity) else { return };
    for descendant in children.iter_descendants(scene_ready.entity) {
        let Ok((id, material_name)) = mesh_materials.get(descendant) else { continue };
        if material_name.0 == "LeatherPartsMat" {
            let Some(material) = asset_materials.get(id.id()) else { continue };
            let mut new_material = material.clone();
            new_material.base_color = color_override.0;
            commands.entity(descendant).insert(MeshMaterial3d(asset_materials.add(new_material)));
        }
    }
}
```

Imports: `bevy::gltf::GltfMaterialName`, `bevy::world_serialization::WorldInstanceReady`.

Source: `examples/gltf/edit_material_on_gltf.rs`

## glTF extras components

The loader attaches JSON `extras` strings as components on spawned entities: `GltfExtras` (node/primitive-level), `GltfSceneExtras`, `GltfMeshExtras`, `GltfMaterialExtras` (all in `bevy::gltf`, each wrapping a raw JSON `String` in `.value`). Query them like any component — the standard channel for authoring-tool metadata (a converter could stash RO object metadata here).

```rust
use bevy::gltf::{GltfExtras, GltfMaterialExtras, GltfMeshExtras, GltfSceneExtras};

fn check(q: Query<(Entity, Option<&Name>, Option<&GltfSceneExtras>, Option<&GltfExtras>,
                   Option<&GltfMeshExtras>, Option<&GltfMaterialExtras>)>) {
    for (id, name, scene_x, x, mesh_x, mat_x) in &q { /* any Some(..) carries JSON */ }
}
```

Source: `examples/gltf/load_gltf_extras.rs`

## Querying primitives/meshes in a spawned scene

Multi-primitive glTF meshes spawn one entity per primitive, each with `Mesh3d`, `MeshMaterial3d<StandardMaterial>`, and `GltfMaterialName`. Filter by material name to locate a specific primitive, then mutate the material or the mesh asset in place (`mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)` yields `VertexAttributeValues` — requires CPU-side mesh data, see loader settings above).

```rust
fn find_top(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(&MeshMaterial3d<StandardMaterial>, &Mesh3d, &GltfMaterialName)>,
) {
    for (mat_handle, mesh_handle, name) in &q {
        if name.0 != "Top" { continue; }
        if let Some(mat) = materials.get_mut(mat_handle) { mat.base_color = Color::from(Hsla::hsl(0.0, 0.9, 0.7)); }
        if let Some(mesh) = meshes.get_mut(mesh_handle)
            && let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        { /* mutate positions */ }
    }
}
```

Source: `examples/gltf/query_gltf_primitives.rs`

## Skinned meshes from glTF

The loader produces a hierarchy where the skinned-mesh entity (with `Mesh3d` + `SkinnedMesh` from `bevy::mesh::skinning`) is a *child* of the mesh node; joints are sibling subtrees. Animate a joint by navigating from the `SkinnedMesh` entity to its parent, then into the joint children, and writing `Transform`.

```rust
use bevy::mesh::skinning::SkinnedMesh;

fn joint_animation(
    time: Res<Time>,
    children: Query<&ChildOf, With<SkinnedMesh>>,
    parents: Query<&Children>,
    mut transforms: Query<&mut Transform>,
) {
    for child_of in &children {
        let mesh_node = child_of.parent();
        let first_joint = parents.get(mesh_node).unwrap()[1];
        let second_joint = parents.get(first_joint).unwrap()[0];
        transforms.get_mut(second_joint).unwrap().rotation =
            Quat::from_rotation_z(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
    }
}
```

Source: `examples/gltf/gltf_skinned_mesh.rs`

## glTF loader extension handlers

`bevy::gltf::extensions::{GltfExtensionHandler, ErasedGltfExtensionHandler, GltfExtensionHandlers}` let you hook into loading itself — the closest thing to writing a custom post-processor without forking the loader. Register a boxed handler by pushing into the `GltfExtensionHandlers` resource (an async lock: `write_blocking()` on native, `write()` + `block_on` on wasm) inside `Plugin::build`. Handlers must implement `dyn_clone`. Hook points seen in the examples:

- `on_spawn_mesh_and_material(load_context, primitive, mesh, material, entity: &mut EntityWorldMut, material_label)` — per spawned primitive; can swap/remove components and add labeled assets via `load_context.add_labeled_asset(label, asset)`.
- `on_animation(load_context, gltf_animation, animation_clip)` — per clip during load; e.g. `animation_clip.add_event_to_target(AnimationTargetId::from_iter(joint_path), time, Event)`.
- `on_animations_collected(load_context, animations, named_animations, animation_roots)` — after all clips; capture handles by name.
- `on_gltf_node(load_context, gltf_node, entity)` — per node as its entity is built; map `gltf_node.index()` to entities.
- `on_scene_completed(load_context, scene, world_root_id, world: &mut World)` — after a scene's world is built; insert components/build assets (e.g. `AnimationGraph::from_clip` stored via `add_labeled_asset`).

```rust
impl Plugin for MyGltfExtPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .resource_mut::<GltfExtensionHandlers>()
            .0.write_blocking()
            .push(Box::new(MyHandler::default()));
    }
}

impl GltfExtensionHandler for MyHandler {
    fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> { Box::new(self.clone()) }
    fn on_spawn_mesh_and_material(&mut self, load_context: &mut LoadContext<'_>,
        _p: &gltf::Primitive, _m: &gltf::Mesh, _mat: &gltf::Material,
        entity: &mut EntityWorldMut, _label: &str) {
        if let Some(mesh3d) = entity.get::<Mesh3d>() {
            let mat = load_context.add_labeled_asset("AColorMaterial".to_string(), CustomMaterial {});
            let mesh = mesh3d.0.clone();
            entity.remove::<Mesh3d>().insert((Mesh2d(mesh), MeshMaterial2d(mat)));
        }
    }
}
```

To fully own materials yourself, also disable the default PBR wiring: `DefaultPlugins.set(PbrPlugin { gltf_enable_standard_materials: false, ..Default::default() })`.

Source: `examples/gltf/gltf_extension_mesh_2d.rs` (mesh/material rewrite), `examples/gltf/gltf_extension_animation_graph.rs` (animation events + graph built at load time)

## Playing a loaded glTF animation (per-entity ready observer)

`.observe(...)` on the `WorldAssetRoot` entity scopes the ready callback to that instance. Find the descendant with `AnimationPlayer`, `play(index).repeat()`, and insert `AnimationGraphHandle` once.

```rust
commands
    .spawn(WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(GLTF_PATH))))
    .observe(play_animation_when_ready);

fn play_animation_when_ready(
    scene_ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<(&mut AnimationPlayer, &AnimationToPlay)>,
) {
    for child in children.iter_descendants(scene_ready.entity) {
        let Ok((mut player, to_play)) = players.get_mut(child) else { continue };
        player.play(to_play.index).repeat();
        commands.entity(child).insert(AnimationGraphHandle(to_play.graph_handle.clone()));
    }
}
```

Source: `examples/gltf/gltf_extension_animation_graph.rs`

## Updating a spawned scene at runtime

Two patterns, no respawn needed:

1. **Move the whole instance** — the sibling `Transform` on the `WorldAssetRoot` entity positions everything spawned under it.
2. **Reach inside** — tag the root with a marker component at spawn, then iterate `children.iter_descendants(root)` and mutate descendant `Transform`s (or any components). Handles for the *same* scene asset can be spawned multiple times as independent instances.

```rust
commands.spawn((
    Transform::from_xyz(-1.0, 0.0, 0.0),
    WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf"))),
));
commands.spawn((WorldAssetRoot(helmet_handle), MovedScene)); // tagged for later

fn move_scene_entities(
    moved_scene: Query<Entity, With<MovedScene>>,
    children: Query<&Children>,
    mut transforms: Query<&mut Transform>,
) {
    for root in &moved_scene {
        for entity in children.iter_descendants(root) {
            if let Ok(mut t) = transforms.get_mut(entity) { t.translation.z += 0.01; }
        }
    }
}
```

To fully respawn: despawn the root entity (children go with it) and spawn a fresh `WorldAssetRoot` with the same handle.

Source: `examples/gltf/update_gltf_scene.rs`
