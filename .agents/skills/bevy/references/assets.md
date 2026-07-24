# Assets, Scenes & Reflection (Bevy 0.19)

Distilled from the Bevy 0.19.0 examples repo. Key 0.19 renames to internalize up front:

- `asset_server.load_with_settings(..)` is gone — use the **`load_builder()`** fluent API (`.with_settings(..)`, `.with_guard(..)`, then `.load(path)`).
- "Scenes" now means **BSN** (`bsn!`, `Scene`, `SceneList`). The old serialized-scene system was renamed to **world serialization**: `DynamicScene` → `DynamicWorld`, `DynamicSceneBuilder` → `DynamicWorldBuilder`, `DynamicSceneRoot` → `DynamicWorldRoot`, `SceneRoot` → `WorldAssetRoot`, `SceneFilter` → `WorldFilter`.
- `AssetEvent<T>` is a **Message** — read with `MessageReader<AssetEvent<T>>`, not `EventReader`.

---

## Custom AssetLoader

`AssetLoader` is an async trait: read bytes from `&mut dyn Reader`, parse, return the asset. Register with `init_asset::<T>()` + `init_asset_loader::<L>()` (or `register_asset_loader(instance)` when the loader needs constructor state — e.g. a GRF archive handle). Use a `thiserror` enum for `type Error`. Source: `examples/asset/custom_asset.rs`.

```rust
#[derive(Asset, TypePath, Debug, Deserialize)]
struct CustomAsset { value: i32 }

#[derive(Default, TypePath)]
struct CustomAssetLoader;

#[derive(Debug, Error)]
enum CustomAssetLoaderError {
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse RON: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),
}

impl AssetLoader for CustomAssetLoader {
    type Asset = CustomAsset;
    type Settings = ();
    type Error = CustomAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes::<CustomAsset>(&bytes)?)
    }

    fn extensions(&self) -> &[&str] { &["custom"] }
}

app.init_asset::<CustomAsset>().init_asset_loader::<CustomAssetLoader>();
```

Extensions are optional (`asset_server.load("data/asset_no_extension")` infers by handle type); multi-dot extensions like `"cool.ron"` (or `"spr"`, `"strfx.ron"`) work. Two loaders can claim the same extension — the requested `Handle<T>` type disambiguates.

### Loader settings

`type Settings` is any `Serialize + Deserialize + Default` struct; it arrives as the second `load()` argument. Callers override per-load via the builder, or persistently via a `.meta` file next to the asset (which must spell out *all* settings fields). Sources: `examples/asset/asset_settings.rs`, `examples/asset/files/bevy_pixel_dark_with_meta.png.meta`.

```rust
let image = asset_server
    .load_builder()
    .with_settings(|s: &mut ImageLoaderSettings| { s.sampler = ImageSampler::nearest(); })
    .load("bevy_pixel_dark_with_settings.png");
```

Gotcha: the first load of a path wins — later loads of the same path with different settings are ignored.

### Labeled sub-assets

Inside `load()`, `load_context.add_labeled_asset("label", sub_asset)` registers a sub-asset addressable as `"path#label"` and returns its `Handle`. Ideal for one-file-many-assets formats (an ACT's frames, a GND's textures, glTF's `GltfAssetLabel::Scene(0)` / `Primitive { mesh, primitive }` labels). Source: `examples/asset/asset_saving_with_subassets.rs` (loader half).

```rust
for (index, one_box) in serialized.boxes.into_iter().enumerate() {
    result_boxes.push(load_context.add_labeled_asset(index.to_string(), one_box));
}
```

### Dependencies from inside a loader

`load_context.load("other/path")` records an untracked dependency handle (loads in parallel, asset completes without waiting). To *wait* for and use a dependency's value during loading, use the load-context builder. Source: `examples/asset/processing/asset_processing.rs` (`CoolTextLoader`).

```rust
// Handle-only dependency (doesn't block this load):
let dep: Handle<Text> = load_context.load(path);

// Awaited, with settings override:
let loaded = load_context
    .load_builder()
    .with_settings(move |s| { *s = settings_override.clone(); })
    .load_value::<Text>(&path)
    .await?;
base_text.push_str(&loaded.get().0);
```

### Nested/wrapped loading (decompression pattern)

A loader can decode bytes and hand them back to the asset system to run *another* loader — the model for GRF-style containers. Source: `examples/asset/asset_decompression.rs`.

```rust
let mut reader = VecReader::new(bytes_uncompressed);
let uncompressed: ErasedLoadedAsset = load_context
    .load_builder()
    .load_untyped_value_from_reader(contained_path, &mut reader)
    .await?;
// Later: uncompressed.take::<Image>()  →  asset_server.add(asset)
```

---

## Asset sources & custom IO

### Extra AssetSource

Register a named source **before** `DefaultPlugins` (AssetPlugin finalizes sources at build). Paths become URL-like: `"example_files://foo.png"`. Source: `examples/asset/extra_source.rs`.

```rust
App::new()
    .register_asset_source(
        "example_files",
        AssetSourceBuilder::platform_default("examples/asset/files", None),
    )
    .add_plugins(DefaultPlugins);

let asset_path = AssetPath::from_path(Path::new("bevy_pixel_light.png"))
    .with_source(AssetSourceId::from("example_files"));
```

### Custom AssetReader

`AssetReader` is the byte-level IO trait (`read`, `read_meta`, `read_directory`, `is_directory`) — it knows storage, not formats. This is the natural home for a GRF-archive-backed source (read entries straight out of the archive). You can also wrap/replace the default source. Source: `examples/asset/custom_asset_reader.rs`.

```rust
struct CustomAssetReader(Box<dyn ErasedAssetReader>);

impl AssetReader for CustomAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        info!("Reading {}", path.display());
        self.0.read(path).await
    }
    // read_meta / read_directory / is_directory delegate similarly
}

app.register_asset_source(
    AssetSourceId::Default,
    AssetSourceBuilder::new(|| Box::new(CustomAssetReader(
        AssetSource::get_default_reader("assets".to_string())(),
    ))),
);
```

There is also a `https://` web source: `DefaultPlugins.set(WebAssetPlugin { .. })` with the `https` feature (`examples/asset/web_asset.rs`).

### Embedded assets

`embedded_asset!` bakes a file into the binary at compile time under the `embedded://` source. Path scheme: `embedded://<crate_name>/<path-with-prefix-omitted>`. Source: `examples/asset/embedded_asset.rs`.

```rust
impl Plugin for EmbeddedAssetPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "examples/asset", "files/bevy_pixel_light.png");
    }
}
// load: asset_server.load("embedded://embedded_asset/files/bevy_pixel_light.png")
```

---

## Asset processing (Loader → Transformer → Saver)

`AssetMode::Processed` runs sources through registered processors into `imported_assets/` (background processing needs the `asset_processor` cargo feature; re-processing on change needs `file_watcher`). A processor may change the asset's *type* (e.g. process `CoolText` → save as plain `Text`). Source: `examples/asset/processing/asset_processing.rs`.

```rust
DefaultPlugins.set(AssetPlugin { mode: AssetMode::Processed, ..default() })

app.register_asset_processor::<LoadTransformAndSave<CoolTextLoader, CoolTextTransformer, CoolTextSaver>>(
        LoadTransformAndSave::new(CoolTextTransformer, CoolTextSaver))
   .set_default_asset_processor::<LoadTransformAndSave<CoolTextLoader, CoolTextTransformer, CoolTextSaver>>("cool.ron");
```

- `AssetTransformer`: `type AssetInput / AssetOutput / Settings`; `async fn transform(TransformedAsset<In>, &Settings) -> TransformedAsset<Out>`.
- `AssetSaver`: `type Asset / Settings / OutputLoader`; `async fn save(&mut Writer, SavedAsset<..>, ..) -> Result<OutputLoader::Settings, _>` — the returned settings are written into the processed `.meta` for the output loader.

### Runtime saving (without the processor)

`save_using_saver` writes an asset through an `AssetSaver` from a running app — do it on `IoTaskPool` (blocking FS, not on Wasm). For assets holding sub-asset handles, build a `SavedAsset` with `SavedAssetBuilder` (`add_labeled_asset_with_new_handle`), and the saver reads them back with `asset.get_labeled_by_id::<Sub>(handle)`. Sources: `examples/asset/asset_saving.rs`, `examples/asset/asset_saving_with_subassets.rs`.

```rust
IoTaskPool::get().spawn(async move {
    save_using_saver(asset_server, &ImageSaver, &ASSET_PATH.into(),
        SavedAsset::from_asset(&image), &ImageSaverSettings::default()).await
}).detach();
```

---

## Hot reloading

Enable the `file_watcher` cargo feature; every filesystem-loaded asset then reloads on change automatically — no code beyond keeping the handle alive. Dependents reload too (a `.meta` edit reprocesses in Processed mode). React to reloads via `AssetEvent::Modified`. Source: `examples/asset/hot_asset_reloading.rs`.

```rust
// Cargo.toml: bevy = { features = ["file_watcher"] }
let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/torus/torus.gltf"));
commands.spawn(WorldAssetRoot(scene)); // edits to the file appear live
```

For custom sources (a GRF reader), hot reload requires your `AssetSourceBuilder` to supply a watcher; `platform_default` paths get one for free.

## Asset events & load-state tracking

`AssetEvent<T>` (Message): `Added`, `Modified`, `Removed`, `Unused`, `LoadedWithDependencies { id }`. `LoadedWithDependencies` on a `LoadedFolder` handle means *everything inside loaded*. Sources: `examples/asset/processing/asset_processing.rs`, `examples/asset/asset_loading.rs`, `examples/scene/world_serialization.rs`.

```rust
fn watch(mut events: MessageReader<AssetEvent<Text>>, texts: Res<Assets<Text>>) {
    for event in events.read() {
        if let AssetEvent::Modified { id } = event { /* re-derive from texts.get(*id) */ }
    }
}

// Polling instead of events:
if let Some(LoadState::Failed(err)) = asset_server.get_load_state(&handle) {
    panic!("Failed to load: {err}"); // fail loudly
}

// Folder loading — keep the handle alive or contents unload:
let folder: Handle<LoadedFolder> = asset_server.load_folder("models/torus");
```

### Waiting on many assets (barrier pattern)

`load_builder().with_guard(guard)` attaches an RAII guard dropped when that load completes — an `Arc`-counted barrier gives you a run condition and an async future for "all N loaded". Source: `examples/asset/multi_asset_sync.rs`.

```rust
let (barrier, guard) = AssetBarrier::new();
let handle = asset_server.load_builder().with_guard(guard.clone()).load("models/a.glb");
// run_if(|b: Option<Res<AssetBarrier>>| b.map(|b| b.is_ready()) == Some(true))
```

---

## Assets<T> mutation & runtime-generated assets

Sources: `examples/asset/generated_assets.rs`, `examples/asset/alter_mesh.rs`.

```rust
// 1. Direct add (immediate handle):
let handle = materials.add(StandardMaterial::default());

// 2. Async generation (deferred insert once the task finishes):
Mesh3d(asset_server.add_async(generate_mesh_async()))   // async fn -> Result<Mesh, E>

// 3. Reserve now, populate later (great for placeholder-until-parsed):
let handle = meshes.reserve_handle();
/* later */ meshes.insert(&handle, mesh).unwrap();

// Mutate one shared asset — affects every entity using it:
let Some(mesh) = meshes.get_mut(&handle) else { return; };
if let Some(VertexAttributeValues::Float32x3(positions)) =
    mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) { /* edit verts */ }

// Swap which asset an entity uses — mutate the handle component instead:
mesh3d.0 = asset_server.load(other_path);   // re-loads are deduped by path

// Take ownership / consume:
let asset = assets.remove(&handle);
```

`ResMut<Assets<T>>::get_mut` flags the asset modified (GPU re-upload); avoid calling it per-frame unless the data really changed — but note the project memory: sprite-layer material writes are an exception.

### RenderAssetUsages

Controls where asset data lives after extraction: `RENDER_WORLD` (GPU only — CPU copy freed, cannot inspect/mutate later), `MAIN_WORLD`, or `all()` (default). Set it in loader settings; custom loaders for meshes/images should expose it in their `Settings`. Source: `examples/asset/alter_mesh.rs`.

```rust
.with_settings(|s: &mut GltfLoaderSettings| s.load_meshes = RenderAssetUsages::all())
```

---

## Scenes

### BSN scenes (the 0.19 "Scene")

Declarative entity trees via `bsn!` / `bsn_list!`; spawn a scene-returning function with `.spawn()` as a system. Source: `examples/scene/bsn.rs` (full patterns in the `bevy-feathers-bsn` skill).

```rust
fn main_scene() -> impl SceneList { bsn_list![Camera2d, ui()] }

fn ui() -> impl Scene {
    bsn! {
        Node { width: percent(100), align_items: AlignItems::Center }
        Children [(
            Button
            on(|_: On<Pointer<Press>>| println!("pressed"))
            Children [( Text("Ok") )]
        )]
    }
}

app.add_systems(Startup, main_scene.spawn());
```

### World serialization (formerly DynamicScene)

Serialize/deserialize entities + resources reflectively to `.scn.ron`. Spawning a `DynamicWorldRoot(Handle<...>)` instantiates the file's entities as children of that root; `WorldAssetRoot` does the same for asset-labeled worlds (e.g. glTF scenes). Types must be registered and `#[reflect(Component)]`/`#[reflect(Resource)]`. Source: `examples/scene/world_serialization.rs`.

```rust
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct ComponentA { x: f32, y: f32 }

#[derive(Component, Reflect)]
#[reflect(Component)]
struct ComponentB {
    value: String,
    #[reflect(skip_serializing)]      // runtime-only field, needs FromWorld/default
    _time_since_startup: Duration,
}

// Load:
commands.spawn(DynamicWorldRoot(asset_server.load("serialized_worlds/level.scn.ron")));

// Save (exclusive system):
fn save(world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let dynamic_world = DynamicWorld::from_world_with(&scene_world, &type_registry.read());
    let ron = dynamic_world.serialize(&type_registry.read()).unwrap();
    IoTaskPool::get().spawn(async move { /* File::create + write */ }).detach();
}
```

### Filtering (WorldFilter / DynamicWorldBuilder)

For control over *what* gets extracted, use `DynamicWorldBuilder` (crate `bevy_world_serialization`, no example file — API from `crates/bevy_world_serialization/src/dynamic_world_builder.rs`):

```rust
let dynamic_world = DynamicWorldBuilder::from_world(world, &type_registry)
    .deny_all()                       // start closed
    .allow_component::<Transform>()   // WorldFilter allow/deny lists
    .allow_component::<ComponentA>()
    .extract_entities(entities.iter())
    .extract_resources()
    .build();
```

---

## Reflection

### Reflect derive & field access

Source: `examples/reflection/reflection.rs`.

```rust
#[derive(Reflect)]
pub struct Foo {
    a: usize,
    nested: Bar,
    #[reflect(ignore)]          // field opted out (then: from_reflect = false, or #[reflect(default)])
    _ignored: NonReflectedValue,
}

*value.get_field_mut::<usize>("a").unwrap() = 2;
let field: &dyn PartialReflect = value.field("a").unwrap();
field.try_downcast_ref::<usize>();          // PartialReflect → concrete
let mut patch = DynamicStruct::default();
patch.insert("a", 4usize);
value.apply(&patch);                        // patch by field name
```

Kind taxonomy (`examples/reflection/reflection_types.rs`): `value.reflect_ref()` matches `ReflectRef::Struct / TupleStruct / Tuple / Enum / List / Array / Map / Set / Function / Opaque`. Force real trait impls with `#[reflect(Hash, PartialEq, Clone)]`; treat a type as a leaf with `#[reflect(opaque)]` (pair with `#[reflect(PartialEq, Clone, Serialize, Deserialize)]`). Generic types must be registered per-instantiation: `app.register_type::<MyType<u32>>()` (`examples/reflection/generic_reflection.rs`).

### ReflectComponent & type data

`#[reflect(Component)]` registers `ReflectComponent` type data — what world serialization, BRP, and `commands.insert_reflect` use to insert a `Box<dyn PartialReflect>` onto an entity. Type data generalizes this: any `Clone` struct named `ReflectMyTrait` with a `FromType<T>` impl, auto-attached via `#[reflect(MyTrait)]`; for object-safe traits `#[reflect_trait]` generates it (with `get`/`get_mut`/`get_boxed` for `&dyn Reflect → &dyn MyTrait`). Source: `examples/reflection/type_data.rs`.

```rust
#[reflect_trait]
trait Health { fn health(&self) -> u32; }

#[derive(Reflect)]
#[reflect(Health)]
struct Skeleton { health: u32 }

let reflect_health = registry.get_type_data::<ReflectHealth>(type_id).unwrap();
let value: &dyn Health = reflect_health.get(value.as_reflect()).unwrap();
```

Useful built-ins to register: `ReflectDefault`, `ReflectFromWorld`, `ReflectComponent`, `ReflectResource`, `ReflectSerialize`, `ReflectDeserialize`.

### Serialization without serde derives

Deriving `Reflect` alone makes a type (de)serializable through the registry — RON or JSON. Deserialization yields a *dynamic* type (`DynamicStruct`); recover the concrete value with `value.apply(&*reflected)` or `FromReflect`. Sources: `examples/reflection/serialization.rs`, `examples/reflection/reflection.rs`.

```rust
let registry = type_registry.read();
let serializer = ReflectSerializer::new(&value, &registry);
let ron_string = ron::ser::to_string_pretty(&serializer, default()).unwrap();

let deserializer = ReflectDeserializer::new(&registry);   // TypedReflectDeserializer if type known
let mut ron_de = ron::de::Deserializer::from_str(&ron_string).unwrap();
let reflected: Box<dyn PartialReflect> = deserializer.deserialize(&mut ron_de).unwrap();
value.apply(&*reflected);
```

### Function reflection

`fn`s, closures, and methods become type-erased callables via `IntoFunction` / `IntoFunctionMut` (feature `reflect_functions`). Supports overloads and manual `DynamicFunction::new` with `SignatureInfo` for exotic signatures — scripting/console-command territory. Source: `examples/reflection/function_reflection.rs`.

```rust
fn add(left: i32, right: i32) -> i32 { left + right }
let function: DynamicFunction = add.into_function();
let args = ArgList::new().with_owned(2_i32).with_owned(2_i32);
let value = function.call(args).unwrap().unwrap_owned();
assert_eq!(value.try_take::<i32>().unwrap(), 4);

// Methods: ArgList::new().with_mut(&mut data) / .with_ref(&data) as receiver.
// Overloads: stringify::<i32>.into_function().with_overload(stringify::<f32>)
```

### Custom attributes

`#[reflect(@expr)]` attaches typed metadata (any `Reflect` value) to types, fields, or enum variants; read back through `TypeInfo` — handy for editor/inspector ranges and tooltips. Keyed by attribute *type* (same type overwrites). Source: `examples/reflection/custom_attributes.rs`.

```rust
#[derive(Reflect)]
struct Slider {
    #[reflect(@0.0..=1.0_f32)]
    value: f32,
}
let TypeInfo::Struct(info) = Slider::type_info() else { panic!() };
let range = info.field("value").unwrap().get_attribute::<RangeInclusive<f32>>().unwrap();
```

---

## glTF notes (brief — full coverage in `gltf.md`)

Sources: `examples/gltf/load_gltf.rs`, `examples/asset/asset_loading.rs`, `examples/asset/alter_mesh.rs`. The glTF loader is the reference implementation of labeled sub-assets: `GltfAssetLabel::Scene(0)`, `::Primitive { mesh, primitive }`, etc., map to `"file.gltf#Scene0"`-style paths. Spawn a scene with `WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/x.gltf")))`; grab one mesh with a `Primitive` label; pass `GltfLoaderSettings` (e.g. `load_meshes = RenderAssetUsages::all()`) through `load_builder().with_settings(..)` when you need CPU-side mesh access. The same label + settings design is the pattern to copy for SPR/ACT/RSM loaders.
