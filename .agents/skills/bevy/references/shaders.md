# Shaders & Custom Materials (Bevy 0.19)

Distilled from the Bevy 0.19 examples in `examples/shader/`, `examples/shader_advanced/`, `examples/ui/ui_material.rs`, and their WGSL in `assets/shaders/`. Key 0.19 changes vs older docs: `ShaderRef` lives in `bevy::shader`; material bind group index in WGSL is the shader-def `#{MATERIAL_BIND_GROUP}` (not hardcoded `@group(2)`); render-world init happens in `RenderStartup` systems (not `FromWorld` + `finish()`); render graph *nodes* are replaced by plain systems added to render schedules (`Core3d`, `RenderGraph`) taking `RenderContext`/`ViewQuery` as system params; bind group layouts are `BindGroupLayoutDescriptor`s resolved via `pipeline_cache.get_bind_group_layout(&desc)`.

## Material trait + AsBindGroup derive

A material = an `Asset` struct deriving `AsBindGroup` + an impl of `Material` (all methods have defaults; override only what you need). Register with `MaterialPlugin::<M>::default()`, use via `MeshMaterial3d<M>` next to `Mesh3d`.

```rust
use bevy::{prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CustomMaterial {
    #[uniform(0)] color: LinearRgba,
    #[texture(1)] #[sampler(2)] color_texture: Option<Handle<Image>>,
    alpha_mode: AlphaMode,            // plain field: not sent to GPU
}
impl Material for CustomMaterial {
    fn fragment_shader() -> ShaderRef { "shaders/custom_material.wgsl".into() }
    fn alpha_mode(&self) -> AlphaMode { self.alpha_mode }
}
app.add_plugins(MaterialPlugin::<CustomMaterial>::default());
```

WGSL side — bindings live in the material bind group, whose index is injected as a shader def; shader-source imports by quoted path also work:

```wgsl
#import bevy_pbr::forward_io::VertexOutput
#import "shaders/custom_material_import.wgsl"::COLOR_MULTIPLIER
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var material_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var material_color_sampler: sampler;
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return material_color * textureSample(material_color_texture, material_color_sampler, mesh.uv);
}
```

`Option<Handle<Image>>` binds a fallback image when `None` — all dimensions supported (`examples/shader/fallback_image.rs`). Custom vertex shaders that need the model matrix import `bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_world}` and `view_transformations::position_world_to_clip`.
Source: `examples/shader/shader_material.rs`, `assets/shaders/custom_material.wgsl`.

### AsBindGroup attribute reference

- `#[uniform(N)]` — field packed into a uniform buffer at binding N. Multiple fields sharing N are merged into one struct (see WebGL2 padding in `examples/shader/extended_material.rs`).
- `#[texture(N)]` with options: `dimension = "1d" | "2d" | "2d_array" | "cube" | "cube_array" | "3d"`, `sample_type = "depth"` (see `examples/shader/array_texture.rs`, `examples/shader/fallback_image.rs`, `examples/shader_advanced/render_depth_to_texture.rs`).
- `#[sampler(N)]` with `sampler_type = "comparison"` for depth compare sampling.
- `#[storage(N, read_only)]` on a `Handle<ShaderBuffer>` — storage buffer (below).
- Struct-level `#[uniform(N, GpuStruct)]` — derive builds the buffer from `impl From<&Material> for GpuStruct` (a `ShaderType`); used to keep GPU layout separate from the CPU asset (`examples/shader/shader_material_bindless.rs`).
- Struct-level `#[bind_group_data(KeyType)]` — non-GPU fields hashed into the specialization key (see Shader defs).
- Struct-level `#[data(N, GpuStruct, binding_array(M))]` and `#[bindless(...)]` — bindless mode (below).

### Data variants: GLSL and WESL shaders

`ShaderRef` accepts `.vert`/`.frag` (GLSL) and `.wesl` sources too — same `Material` impl, just point `vertex_shader()`/`fragment_shader()` at them. WESL supports cross-file imports via a held `Handle<Shader>` to the utility module. Source: `examples/shader/shader_material_glsl.rs`, `examples/shader/shader_material_wesl.rs`.

## ExtendedMaterial (extend StandardMaterial)

Wrap `StandardMaterial` with an extension that only adds bindings ≥ 100 (0–99 reserved for the base). You get the full PBR pipeline and hook in before/after lighting.

```rust
use bevy::pbr::{ExtendedMaterial, MaterialExtension};

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
struct MyExtension { #[uniform(100)] quantize_steps: u32 }
impl MaterialExtension for MyExtension {
    fn fragment_shader() -> ShaderRef { "shaders/extended_material.wgsl".into() }
    fn deferred_fragment_shader() -> ShaderRef { "shaders/extended_material.wgsl".into() }
}
app.add_plugins(MaterialPlugin::<ExtendedMaterial<StandardMaterial, MyExtension>>::default());
// spawn: MeshMaterial3d(materials.add(ExtendedMaterial { base: StandardMaterial {..}, extension: MyExtension::new(1) }))
```

The canonical extension shader pattern (works in forward and deferred via `PREPASS_PIPELINE`):

```wgsl
#import bevy_pbr::{pbr_fragment::pbr_input_from_standard_material, pbr_functions::alpha_discard}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{prepass_io::{VertexOutput, FragmentOutput}, pbr_deferred_functions::deferred_output}
#else
#import bevy_pbr::{forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}}
#endif
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> my_extended_material: MyExtendedMaterial;
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color.b = pbr_input.material.base_color.r;   // pre-lighting tweak
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);                            // post-lighting tweak here
    out.color = main_pass_post_lighting_processing(pbr_input, out.color); // fog/tonemap/deband
#endif
    return out;
}
```

Source: `examples/shader/extended_material.rs`, `assets/shaders/extended_material.wgsl`.

## Bindless materials

`#[bindless(limit(N))]` groups up to N materials per bind group; textures/samplers become binding arrays and plain data goes in a storage-buffer array via the struct-level uniform + `binding_array(slot)`:

```rust
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[uniform(0, BindlessMaterialUniform, binding_array(10))]
#[bindless(limit(4))]
struct BindlessMaterial { color: LinearRgba, #[texture(1)] #[sampler(2)] color_texture: Option<Handle<Image>> }
```

WGSL branches on the `BINDLESS` shader def; the per-instance slot comes from the mesh uniform, and index tables map slots to slab indices:

```wgsl
#ifdef BINDLESS
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage> materials: array<MaterialBindings>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var<storage> material_color: binding_array<Color>;
// sample: bindless_textures_2d[materials[slot].color_texture] with slot =
//   mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
#else
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: Color;
#endif
```

Bindless *extensions* must relocate their index table: `#[data(50, MyUniform, binding_array(101))]` + `#[bindless(index_table(range(50..53), binding(100)))]` so they don't collide with StandardMaterial's table at binding 0. Source: `examples/shader/shader_material_bindless.rs`, `assets/shaders/bindless_material.wgsl`, `examples/shader/extended_material_bindless.rs`, `assets/shaders/extended_material_bindless.wgsl`.

## Shader defs + material specialization

Per-material pipeline permutations: put non-GPU state into a hashable key via `#[bind_group_data(Key)]`, then push shader defs in `Material::specialize`. Keep keys tiny — they're hashed per drawn entity.

```rust
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(CustomMaterialKey)]
struct CustomMaterial { #[uniform(0)] color: LinearRgba, is_red: bool }

#[repr(C)] #[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct CustomMaterialKey { is_red: bool }
impl From<&CustomMaterial> for CustomMaterialKey { /* copy flags */ }

impl Material for CustomMaterial {
    fn fragment_shader() -> ShaderRef { "shaders/shader_defs.wgsl".into() }
    fn specialize(
        _pipeline: &MaterialPipeline,                 // NOT generic in 0.19
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.is_red {
            descriptor.fragment.as_mut().unwrap().shader_defs.push("IS_RED".into());
        }
        Ok(())
    }
}
```

WGSL: `#ifdef IS_RED ... #else ... #endif`. Value-carrying defs: `ShaderDefVal::Bool("PARTY_MODE".to_string(), flag)` (`examples/shader/shader_material_wesl.rs`). Source: `examples/shader/shader_defs.rs`, `assets/shaders/shader_defs.wgsl`.

## 2D materials (Material2d)

Same derive, different trait/plugin/components — note the 0.19 module: `bevy::sprite_render`.

```rust
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};
impl Material2d for CustomMaterial {
    fn fragment_shader() -> ShaderRef { "shaders/custom_material_2d.wgsl".into() }
    fn alpha_mode(&self) -> AlphaMode2d { AlphaMode2d::Mask(0.5) }
}
app.add_plugins(Material2dPlugin::<CustomMaterial>::default());
// spawn: (Mesh2d(meshes.add(Rectangle::default())), MeshMaterial2d(materials.add(..)))
```

WGSL imports change: `#import bevy_sprite::mesh2d_vertex_output::VertexOutput`; for vertex shaders use `bevy_sprite::mesh2d_functions` (`mesh2d_position_local_to_clip`). `Material2d` also has a `specialize` hook with `Material2dKey`. Source: `examples/shader/shader_material_2d.rs`, `assets/shaders/custom_material_2d.wgsl`.

## UiMaterial

```rust
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct CustomUiMaterial {
    #[uniform(0)] color: Vec4,
    #[uniform(1)] slider: Vec4,          // Vec4 for WebGL2 16-byte alignment
    #[texture(2)] #[sampler(3)] color_texture: Handle<Image>,
    #[uniform(4)] border_color: Vec4,
}
impl UiMaterial for CustomUiMaterial {
    fn fragment_shader() -> ShaderRef { "shaders/custom_ui_material.wgsl".into() }
}
app.add_plugins(UiMaterialPlugin::<CustomUiMaterial>::default());
// spawn: (Node { border, border_radius, .. }, MaterialNode(ui_materials.add(..)))
```

WGSL: UI materials bind at literal `@group(1)`; the vertex output carries UI geometry — `in.uv`, `in.size`, `in.border_widths` (vec4), `in.border_radius` (vec4):

```wgsl
#import bevy_ui::ui_vertex_output::UiVertexOutput
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@fragment fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> { ... }
```

Source: `examples/ui/ui_material.rs`, `assets/shaders/custom_ui_material.wgsl`.

## Animated shaders: the globals binding

No CPU work needed for time-driven effects — `globals.time` ships with the view bindings:

```wgsl
#import bevy_pbr::{mesh_view_bindings::globals, forward_io::VertexOutput}
@fragment fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = sin(globals.time * 2.0) * 0.5 + 0.5; ...
}
```

An empty material (`struct CustomMaterial {}` deriving `AsBindGroup`) is fine. Source: `examples/shader/animate_shader.rs`, `assets/shaders/animate_shader.wgsl`. Screen-space UVs from the view uniform: `examples/shader/shader_material_screenspace_texture.rs`.

## Alpha modes & the prepass

`Material::alpha_mode(&self)` per-instance (`Opaque`/`Mask(cutoff)`/`Blend`, plus `AlphaMode2d` in 2d). Blend-mode materials are skipped by the depth/normal/motion-vector prepasses. Enable prepasses by adding `DepthPrepass`, `NormalPrepass`, `MotionVectorPrepass` components to the camera; a material can opt out with `fn enable_prepass() -> bool { false }` or override `prepass_fragment_shader()`. `Material::specialize` is also used for the prepass pipeline. Source: `examples/shader/shader_prepass.rs`.

## Custom vertex attributes

Define a `MeshVertexAttribute` with a high random id, insert the data on the mesh, and build the vertex buffer layout in `specialize`:

```rust
const ATTRIBUTE_BLEND_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("BlendColor", 988540917, VertexFormat::Float32x4);
let mesh = Mesh::from(Cuboid::default())
    .with_inserted_attribute(ATTRIBUTE_BLEND_COLOR, vec![[1.0, 0.0, 0.0, 1.0]; 24]);
// in Material::specialize:
let vertex_layout = layout.0.get_layout(&[
    Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
    ATTRIBUTE_BLEND_COLOR.at_shader_location(1),
])?;
descriptor.vertex.buffers = vec![vertex_layout];
```

WGSL vertex input: `@location(1) blend_color: vec4<f32>` (plus `@builtin(instance_index)` for `get_world_from_local`). Source: `examples/shader_advanced/custom_vertex_attribute.rs`, `assets/shaders/custom_vertex_attribute.wgsl`.

## Automatic instancing + MeshTag

Same `Mesh3d` handle + same material handle ⇒ one draw call, no extra code (works with any material incl. StandardMaterial). Per-instance data rides on `MeshTag(u32)` — read it in the shader:

```wgsl
#import bevy_pbr::mesh_functions
let tag = mesh_functions::get_tag(vertex.instance_index);   // or mesh.instance_index in fragment
```

Uses: index into a texture (`examples/shader/automatic_instancing.rs`), pick an array-texture layer (`examples/shader/array_texture.rs`, `#[texture(0, dimension = "2d_array")]`), or index a storage buffer of per-instance colors (`examples/shader/storage_buffer.rs`).

## Storage buffers in materials

`ShaderBuffer` is an asset (`bevy::render::storage`); bind by handle, mutate at runtime through `Assets<ShaderBuffer>`:

```rust
let colors = buffers.add(ShaderBuffer::from(color_data));   // Vec<[f32;4]>
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CustomMaterial { #[storage(0, read_only)] colors: Handle<ShaderBuffer> }
// update: buffers.get_mut(&material.colors).unwrap().set_data(new_vec);
```

WGSL: `@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage, read> colors: array<vec4<f32>, 5>;`. Source: `examples/shader/storage_buffer.rs`, `assets/shaders/storage_buffer.wgsl`.

## Post-processing

Two routes in 0.19:

**Easy route — `FullscreenMaterial`** (`bevy::core_pipeline::fullscreen_material`): a `Component + ExtractComponent + ShaderType` on the camera; the plugin does extraction, pipeline, and pass for you.

```rust
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
struct FullscreenEffect { intensity: f32 }
impl FullscreenMaterial for FullscreenEffect {
    fn fragment_shader() -> ShaderRef { "shaders/fullscreen_effect.wgsl".into() }
    // override schedule()/schedule_configs() to run in Core2d instead of the default Core3d
}
app.add_plugins(FullscreenMaterialPlugin::<FullscreenEffect>::default());
```

**Manual route** — no render-graph node structs anymore; add a *system* to the `Core3d` schedule in `Core3dSystems::PostProcess`, using `RenderContext` and `ViewQuery` as system params:

```rust
// build():
app.add_plugins((ExtractComponentPlugin::<PostProcessSettings>::default(),
                 UniformComponentPlugin::<PostProcessSettings>::default()));
render_app.add_systems(RenderStartup, init_post_process_pipeline);
render_app.add_systems(Core3d, post_process_system.in_set(Core3dSystems::PostProcess));

// init: BindGroupLayoutDescriptor::new("...", &BindGroupLayoutEntries::sequential(
//   ShaderStages::FRAGMENT, (texture_2d(TextureSampleType::Float { filterable: true }),
//   sampler(SamplerBindingType::Filtering), uniform_buffer::<PostProcessSettings>(true))));
// vertex state: fullscreen_shader.to_vertex_state()  (Res<FullscreenShader>)
// pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor { layout: vec![layout.clone()], .. });

fn post_process_system(
    view: ViewQuery<(&ViewTarget, &PostProcessSettings, &DynamicUniformIndex<PostProcessSettings>)>,
    ..., mut ctx: RenderContext,
) {
    let post_process = view_target.post_process_write();  // source + destination; MUST write destination
    // create bind group here (source view flips every write, so it can't be prepared earlier);
    // cache it keyed on post_process.source.id()
    let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
        color_attachments: &[Some(RenderPassColorAttachment { view: post_process.destination, .. })], ..});
    pass.set_bind_group(0, bind_group, &[settings_index.index()]);  // dynamic uniform per view
    pass.draw(0..3, 0..1);                                          // fullscreen triangle
}
```

WGSL: `#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput`, sample `screen_texture` at `in.uv`. Match the `ColorTargetState::format` to the camera (Rgba8UnormSrgb non-HDR; change if HDR/bloom). Source: `examples/shader_advanced/custom_post_processing.rs`, `assets/shaders/post_processing.wgsl`, `examples/shader_advanced/fullscreen_material.rs`.

## Compute shaders

Compute also runs as a system, added to the `RenderGraph` schedule (ordered `.before(camera_driver)`), with pipeline setup in `RenderStartup` and bind groups in `Render`/`RenderSystems::PrepareBindGroups`:

```rust
render_app
    .add_systems(RenderStartup, init_pipeline)
    .add_systems(Render, prepare_bind_group.in_set(RenderSystems::PrepareBindGroups))
    .add_systems(RenderGraph, run_compute.before(camera_driver));

// init: layout = BindGroupLayoutDescriptor::new("...", &BindGroupLayoutEntries::sequential(
//         ShaderStages::COMPUTE, (texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::ReadOnly),
//                                 texture_storage_2d(..., WriteOnly), uniform_buffer::<MyUniforms>(false))));
// pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
//     layout: vec![layout.clone()], shader, entry_point: Some(Cow::from("update")), ..default() });

fn run_compute(mut ctx: RenderContext, cache: Res<PipelineCache>, p: Res<MyPipeline>, bg: Res<MyBindGroups>) {
    let Some(pipeline) = cache.get_compute_pipeline(p.id) else { return };  // may still be compiling
    let mut pass = ctx.command_encoder().begin_compute_pass(&ComputePassDescriptor::default());
    pass.set_bind_group(0, &bg.0, &[]);
    pass.set_pipeline(pipeline);
    pass.dispatch_workgroups(SIZE.x / 8, SIZE.y / 8, 1);
}
```

Storage-texture images need `TextureUsages::STORAGE_BINDING` (+ `COPY_DST`/`TEXTURE_BINDING` as needed) and `RenderAssetUsages::RENDER_WORLD`; ping-pong two images and swap which the `Sprite` displays. Main-world data reaches the render world via `ExtractResourcePlugin::<T>` (T: `Clone + ExtractResource`). Poll `pipeline_cache.get_compute_pipeline_state` to fail loudly on shader errors. Source: `examples/shader/compute_shader_game_of_life.rs`. Writing mesh vertex/index slabs directly from compute (via `MeshAllocator` slices + `MeshAllocatorSettings::extra_buffer_usages = BufferUsages::STORAGE`): `examples/shader_advanced/compute_mesh.rs`.

### GPU readback

Spawn a `Readback` component and observe `ReadbackComplete` — async, fires every frame until despawned:

```rust
let mut buffer = ShaderBuffer::from((0..16u32).collect::<Vec<u32>>());
buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
commands.spawn(Readback::buffer(buffers.add(buffer)))
    .observe(|event: On<ReadbackComplete>| {
        let data: Vec<u32> = event.to_shader_type();
    });
// also: Readback::buffer_range(handle, offset, size) and Readback::texture(image_handle)
```

Images being read back need `TextureUsages::COPY_SRC` (`Image::new_uninit` for GPU-only targets). Source: `examples/shader/gpu_readback.rs`.

## Custom instancing (custom render phase entries)

For per-instance data beyond `MeshTag`, bypass materials: extract an instance-data component, upload it as a vertex buffer stepped per-instance, extend `MeshPipeline`'s descriptor, and enqueue a custom `RenderCommand` into `Transparent3d`.

```rust
// plugin (render app):
.add_render_command::<Transparent3d, DrawCustom>()
.init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
.add_systems(RenderStartup, init_custom_pipeline.after(MeshPipelineSystems))
.add_systems(Render, (queue_custom.in_set(RenderSystems::QueueMeshes),
                      prepare_instance_buffers.in_set(RenderSystems::PrepareResources)));

impl SpecializedMeshPipeline for CustomPipeline {
    type Key = MeshPipelineKey;
    fn specialize(&self, key: Self::Key, layout: &MeshVertexBufferLayoutRef)
        -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;   // start from Bevy's
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: size_of::<InstanceData>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![/* Float32x4 @ location 3, Float32x4 @ location 4 */],
        });
        descriptor.fragment.as_mut().unwrap().shader = self.shader.clone();
        Ok(descriptor)
    }
}

type DrawCustom = (SetItemPipeline, SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>, SetMeshBindGroup<2>, DrawMeshInstanced);
```

The `RenderCommand::render` impl fetches mesh slices from `MeshAllocator`, sets vertex buffer 0 (mesh) and 1 (instances), then `draw_indexed(.., 0..instance_count)`. Gotchas: the instanced entity needs `NoFrustumCulling` (Bevy culls by the single mesh's Aabb), the camera needs `NoIndirectDrawing` (direct draws), the component needs `impl SyncComponent` + `ExtractComponent`, and queueing reads `ViewKeyCache` per `retained_view_entity` and pushes with `transparent_phase.add_retained(Transparent3d { sorting_info: TransparentSortingInfo3d::Sorted { mesh_center, depth_bias }, .. })`. WGSL: instance attrs are plain `@location(3)/@location(4)` inputs. Source: `examples/shader_advanced/custom_shader_instancing.rs`, `assets/shaders/instancing.wgsl`.

## Specialized mesh pipelines (full pipeline control, no Material)

Render a `Mesh3d` with a hand-built `RenderPipelineDescriptor`. Marker component needs visibility wiring:

```rust
#[derive(Clone, Component, ExtractComponent)]
#[require(VisibilityClass)]
#[component(on_add = visibility::add_visibility_class::<CustomRenderedEntity>)]
struct CustomRenderedEntity;
```

`specialize` builds vertex layout from the mesh (`layout.0.get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0), ...])`), gets view bind group layouts from `mesh_pipeline.get_view_layout(MeshPipelineViewLayoutKey::from(mesh_key))`, and sets `depth_stencil` with `CORE_3D_DEPTH_FORMAT`, `depth_write_enabled: Some(true)`, `CompareFunction::GreaterEqual` (reverse-Z). Draw commands: `(SetItemPipeline, SetMeshViewBindGroup<0>, SetMeshViewEmptyBindGroup<1>, SetMeshBindGroup<2>, DrawMesh)`. The queue system (in `RenderSystems::Queue`) is retained-render-world aware: it must use `DirtySpecializations::iter_to_dequeue/iter_to_queue` plus a `PendingQueues`-wrapping resource, and bin via `opaque_phase.add(Opaque3dBatchSetKey {..}, Opaque3dBinKey { asset_id }, entity_pair, uniform_index, BinnedRenderPhaseType::mesh(..))`. Source: `examples/shader_advanced/specialized_mesh_pipeline.rs`.

## Custom phase items & the Variants/Specializer API

For non-mesh GPU drawing inside Bevy's phases: upload raw `RawBufferVec<Vertex>` buffers (`write_buffer(device, queue)`), and specialize a raw `RenderPipeline` with the 0.19 `Variants` API instead of `SpecializedRenderPipeline`:

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, SpecializerKey)]
struct CustomPhaseKey(Msaa);
impl Specializer<RenderPipeline> for CustomPhaseSpecializer {
    type Key = CustomPhaseKey;
    fn specialize(&self, key: Self::Key, descriptor: &mut RenderPipelineDescriptor)
        -> Result<Canonical<Self::Key>, BevyError> {
        descriptor.multisample.count = key.0.samples();
        Ok(key)
    }
}
// resource: Variants::new(CustomPhaseSpecializer, base_descriptor);
// queue: pipeline.variants.specialize(&pipeline_cache, CustomPhaseKey(*msaa))
// bin with BinnedRenderPhaseType::NonMesh and AssetId::<Mesh>::invalid().untyped()
```

Source: `examples/shader_advanced/custom_phase_item.rs`, `assets/shaders/custom_phase_item.wgsl`.

## Custom render phase (your own `Stencil3d`-style pass)

Full recipe for a brand-new phase: define a `PhaseItem` + `SortedPhaseItem` + `CachedRenderPipelinePhaseItem` struct; register `DrawFunctions<Stencil3d>`, `ViewSortedRenderPhases<Stencil3d>`, `SortedRenderPhasePlugin::<Stencil3d, MeshPipeline>`, and `add_render_command`. Systems: `extract_camera_phases` (ExtractSchedule — insert a phase per camera's `retained_view_entity`), queue in `RenderSystems::QueueMeshes`, `sort_phase_system::<Stencil3d>` in `RenderSystems::PhaseSort`, `batch_and_prepare_sorted_render_phase::<Stencil3d, StencilPipeline>` in `RenderSystems::PrepareResources` (requires `GetBatchData`/`GetFullBatchData` impls for batching). The pass itself is a system in the `Core3d` schedule:

```rust
render_app.add_systems(Core3d, custom_draw_system.after(main_opaque_pass_3d).in_set(Core3dSystems::MainPass));

fn custom_draw_system(world: &World, view: ViewQuery<(&ExtractedCamera, &ExtractedView, &ViewTarget, ...)>,
    stencil_phases: Res<ViewSortedRenderPhases<Stencil3d>>, mut ctx: RenderContext) {
    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        color_attachments: &[Some(target.get_color_attachment())], depth_stencil_attachment: None, ..});
    stencil_phase.render(&mut render_pass, world, view.entity())?;
}
```

Source: `examples/shader_advanced/custom_render_phase.rs`.

## Manual AsBindGroup / manual materials

- **Texture binding arrays** (many textures, one binding): implement `AsBindGroup` by hand — return `Err(AsBindGroupError::CreateBindGroupDirectly)` from `unprepared_bind_group`, build the bind group in `as_bind_group` with `BindGroupEntries::sequential((&texture_view_slice[..], &sampler))`, declare layout entries with `.count(NonZero::<u32>::new(N))`, fill gaps with `FallbackImage`, and gate on `WgpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING`. Return `AsBindGroupError::RetryNextUpdate` while images load. Source: `examples/shader_advanced/texture_binding_array.rs`.
- **Fully manual material** (no `Material` trait at all): `ErasedRenderAssetPlugin`, a `MaterialBindGroupAllocator` registered in `MaterialBindGroupAllocators` keyed by `TypeId`, hand-written extract systems feeding `RenderMaterialInstances`, and `DirtySpecializationSystems::CheckForChanges/CheckForRemovals`. Rarely worth it; see `examples/shader_advanced/manual_material.rs`.
- **Depth textures in materials**: `#[texture(0, sample_type = "depth")] #[sampler(1, sampler_type = "comparison")]`; copy the camera's `ViewDepthTexture` to an `Image` in a `Core3d` system between `Core3dSystems::Prepass` and `MainPass` (can't sample a live depth buffer). Source: `examples/shader_advanced/render_depth_to_texture.rs`.

## Quick reference: render-world scheduling

| When | Where |
|---|---|
| Create pipelines/layouts once | `RenderStartup` system (after `MeshPipelineSystems` if cloning `MeshPipeline`) |
| Extract main→render world | `ExtractSchedule`, or `ExtractComponentPlugin` / `ExtractResourcePlugin` / `UniformComponentPlugin` |
| Upload buffers / per-frame prep | `Render` in `RenderSystems::PrepareResources` |
| Create bind groups | `Render` in `RenderSystems::PrepareBindGroups` |
| Enqueue phase items | `Render` in `RenderSystems::Queue` / `QueueMeshes` |
| Sort/batch sorted phases | `RenderSystems::PhaseSort` / `PrepareResources` |
| Camera-driven passes | `Core3d` (or `Core2d`) schedule, in a `Core3dSystems` set |
| Camera-independent GPU work (compute) | `RenderGraph` schedule, `.before(camera_driver)` |
