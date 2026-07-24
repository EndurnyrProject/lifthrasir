# 2D & 3D Rendering, Cameras, Transforms, Gizmos (Bevy 0.19)

Distilled from the official Bevy examples. Everything below is component-based (no bundles): spawn `Camera2d`, `Camera3d`, `Sprite`, `Mesh3d`, `MeshMaterial3d`, etc. directly. Events are Messages (`MessageReader`/`MessageWriter`). UI lengths use `px(12)` / `percent(100)` helpers.

## 0.19 renames worth memorizing

- `PointLight/SpotLight/DirectionalLight { shadow_maps_enabled: true }` (was `shadows_enabled`).
- Ambient light resource is `GlobalAmbientLight` (was `AmbientLight`); `GlobalAmbientLight::NONE` to disable.
- HDR is a marker component `bevy::camera::Hdr` on the camera, not a `Camera` field.
- Camera render target is a separate component: `RenderTarget::Image(handle.into())` spawned alongside `Camera` (was `Camera::target`).
- `Skybox { image: Some(handle), brightness, .. }` — `image` is an `Option`.
- `TextFont { font: handle.into(), font_size: FontSize::Px(50.0), .. }`; justification enum is `Justify`.
- glTF scenes spawn via `WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("m.gltf")))` (was `SceneRoot`).
- Math helpers: `bevy::math::ops::{sin, cos, powf}` instead of bare `f32::sin` in examples.
- Camera modules moved: `bevy::camera::{Viewport, ScalingMode, SubCameraView, RenderTarget, Exposure, visibility::RenderLayers}`.

---

## Sprites

### Basic sprite, tint, flip, custom size
`Sprite` is the component; transparency comes from the color's alpha; 2D draw order is the Z translation. `examples/2d/sprite.rs`, `examples/2d/sprite_flipping.rs`, `examples/2d/transparency_2d.rs`

```rust
commands.spawn(Camera2d);
commands.spawn((
    Sprite {
        image: asset_server.load("branding/icon.png"),
        color: Color::srgba(0.0, 0.0, 1.0, 0.7), // tint * alpha
        flip_x: true,
        custom_size: Some(Vec2::splat(160.0)),
        ..default()
    },
    Transform::from_xyz(0.0, 0.0, 0.1), // higher z renders on top (2D)
));
```

For pixel art, prevent blur globally: `DefaultPlugins.set(ImagePlugin::default_nearest())`.

### Sprite sheet animation
Atlas lives inside `Sprite.texture_atlas`; animate by mutating `atlas.index`. `examples/2d/sprite_sheet.rs`, `examples/2d/sprite_animation.rs`

```rust
let layout = layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(24), 7, 1, None, None));
commands.spawn((
    Sprite::from_atlas_image(texture, TextureAtlas { layout, index: 1 }),
    AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
));

fn animate(time: Res<Time>, mut q: Query<(&mut AnimationTimer, &mut Sprite)>) {
    for (mut timer, mut sprite) in &mut q {
        timer.tick(time.delta());
        if timer.just_finished() && let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = if atlas.index == LAST { FIRST } else { atlas.index + 1 };
        }
    }
}
```

### Atlas built from a folder at runtime
`TextureAtlasBuilder` packs loaded images; add padding to stop linear-sampling bleed. Gate on `AssetEvent<LoadedFolder>::is_loaded_with_dependencies`. `examples/2d/texture_atlas.rs`

```rust
let mut builder = TextureAtlasBuilder::default();
builder.padding(UVec2::new(6, 6));
for handle in folder.handles.iter() {
    let id = handle.id().typed_unchecked::<Image>();
    builder.add_texture(Some(id), textures.get(id).unwrap());
}
let (layout, sources, image) = builder.build().unwrap();
// per-sprite lookup: sources.handle(layout_handle, &original_image_handle)
```

### Nine-slice, tiling, fit/fill — `SpriteImageMode`
`examples/2d/sprite_slice.rs`, `examples/2d/sprite_tile.rs`, `examples/2d/sprite_scale.rs`

```rust
Sprite {
    image, custom_size: Some(Vec2::new(100.0, 200.0)),
    image_mode: SpriteImageMode::Sliced(TextureSlicer {
        border: BorderRect::all(200.0),
        center_scale_mode: SliceScaleMode::Tile { stretch_value: 0.5 },
        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 0.2 },
        max_corner_scale: 0.2,
    }),
    ..default()
}
// Whole-sprite tiling:
SpriteImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: 0.5 }
// Aspect-preserving scaling inside custom_size:
SpriteImageMode::Scale(SpriteScalingMode::FitCenter) // also FillCenter/FillStart/FillEnd/FitStart/FitEnd
```

---

## Text2d (world-space text)

`Text2d` renders in the 2D world (usable as sprite children, e.g. labels under units). `examples/2d/text2d.rs`

```rust
use bevy::sprite::{Anchor, Text2dShadow};
use bevy::text::{LineBreak, TextBounds, FontSmoothing};

commands.spawn((
    Text2d::new("wrapped label"),
    TextFont { font: font.clone().into(), font_size: FontSize::Px(35.0), ..default() },
    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
    TextBounds::from(Vec2::new(300.0, 200.0)),  // wrap box
    TextBackgroundColor(Color::BLACK.with_alpha(0.5)),
    Text2dShadow::default(),
    Anchor::TOP_CENTER,                          // anchor constants are assoc consts
    Transform::from_translation(Vec3::Z),        // above the parent sprite
));
```

- Rich text: child entities with `TextSpan` + per-span `TextFont`/`TextColor`.
- Prefer changing `font_size` over `Transform::scale` (scale pixelates the quad).
- `FontSmoothing::None` via `text_font.with_font_smoothing(...)` for crisp bitmap-style text.

---

## 2D Meshes

### Mesh2d + ColorMaterial
`examples/2d/mesh2d.rs`, `examples/2d/2d_shapes.rs`

```rust
commands.spawn((
    Mesh2d(meshes.add(Circle::new(50.0))),        // any primitive: Rectangle, Annulus, Capsule2d,
    MeshMaterial2d(materials.add(Color::hsl(180., 0.95, 0.7))), // RegularPolygon, Triangle2d, .to_ring(w)...
    Transform::from_xyz(x, y, 0.0),
));
```

### 2D alpha modes & depth
`ColorMaterial` has `AlphaMode2d::{Opaque, Mask(f32), Blend}` (`bevy::sprite_render::AlphaMode2d`). Opaque/Mask 2D meshes use the depth buffer; Blend sorts by Z. `examples/2d/mesh2d_alpha_mode.rs`

```rust
MeshMaterial2d(materials.add(ColorMaterial {
    color: WHITE.into(),
    alpha_mode: AlphaMode2d::Mask(0.5),
    texture: Some(texture_handle.clone()),
    ..default()
}))
```

### Repeated / transformed textures
Default sampler clamps to edge; opt into repeat per-load, then scale UVs with the material's `uv_transform` (full `Affine2`). `examples/2d/mesh2d_repeated_texture.rs`

```rust
let img = asset_server.load_builder()
    .with_settings(|s: &mut ImageLoaderSettings| {
        s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..default()
        });
    })
    .load("path.png");
ColorMaterial { texture: Some(img), uv_transform: Affine2::from_scale(Vec2::new(2., 3.)), ..default() }
```

Wireframes: `Wireframe2dPlugin` + `Wireframe2dConfig.global` (2D), `WireframePlugin` + `WireframeConfig` (3D); not on wasm. `examples/2d/2d_shapes.rs`, `examples/3d/3d_shapes.rs`

---

## 3D Meshes & Custom Mesh Generation

### Primitives
`meshes.add(shape)` — `Mesh: From<primitive>`. `examples/3d/3d_shapes.rs`, `examples/3d/3d_scene.rs`

```rust
commands.spawn((
    Mesh3d(meshes.add(Sphere::default().mesh().ico(5).unwrap())), // or .uv(32, 18)
    MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    Transform::from_xyz(0.0, 0.5, 0.0),
));
meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10));
meshes.add(Extrusion::new(Annulus::default(), 1.));   // 2D primitive -> volume
```

### Hand-built mesh (terrain-style)
Counter-clockwise winding faces the viewer; normals required for lighting; keep `MAIN_WORLD` usage if you'll mutate later. `examples/3d/generate_custom_mesh.rs`

```rust
use bevy::{asset::RenderAssetUsages, mesh::{Indices, VertexAttributeValues},
           render::render_resource::PrimitiveTopology};

let mesh = Mesh::new(PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)  // Vec<[f32; 3]>
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)            // Vec<[f32; 2]>, (0,0)=top-left
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices));

// Runtime mutation:
let mesh = meshes.get_mut(&handle).unwrap();
let VertexAttributeValues::Float32x2(uvs) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0).unwrap()
    else { panic!("expected Float32x2") };
```

Runtime textures: `Image::new_fill(Extent3d { .. }, TextureDimension::D2, &bytes, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)`. `examples/3d/3d_shapes.rs`

---

## StandardMaterial & Transparency

### Common knobs
`examples/3d/lighting.rs`, `examples/3d/fog.rs`

```rust
StandardMaterial {
    base_color: Color::WHITE,
    base_color_texture: Some(handle),
    perceptual_roughness: 1.0,       // 0 = mirror, 1 = diffuse
    metallic: 0.5,
    reflectance: 1.0,
    emissive: LinearRgba::new(4.0, 0.0, 0.0, 0.0), // HDR values glow with Bloom
    unlit: true,                     // skip lighting entirely (sky domes, sprites)
    cull_mode: None,                 // double-sided (billboards, thin geometry)
    alpha_mode: AlphaMode::Mask(0.5),
    ..default()
}
```

### Alpha modes
`examples/3d/transparency_3d.rs`, `examples/3d/blend_modes.rs`
- `Opaque` — default; ignores alpha.
- `Mask(cutoff)` — binary cutout, still writes depth; ideal for sprite billboards (no sort issues).
- `Blend` — smooth alpha; back-to-front sorted per entity; classic transparency-ordering pain.
- `AlphaToCoverage` — stepped alpha via MSAA samples; depth-friendly cutout AA.
- `Premultiplied`, `Add`, `Multiply` — effect blends.
- `Color::srgba(..)` with alpha < 1.0 converted `.into()` a material auto-selects `Blend`.
- Order-independent transparency exists as an opt-in camera setup (`examples/3d/order_independent_transparency.rs`) if sorted blend artifacts bite.

---

## Lighting & Shadows

### Light types
`examples/3d/lighting.rs`, `examples/3d/spotlight.rs`

```rust
commands.spawn((
    PointLight { intensity: 100_000.0, color: RED.into(), range: 20.0,
                 shadow_maps_enabled: true, shadow_depth_bias: 0.2, ..default() },
    Transform::from_xyz(1.0, 2.0, 0.0),
));
commands.spawn((
    SpotLight { intensity: 100_000.0, inner_angle: 0.6, outer_angle: 0.8,
                shadow_maps_enabled: true, ..default() },
    Transform::from_xyz(-1.0, 2.0, 0.0).looking_at(target, Vec3::Z),
));
commands.spawn((
    DirectionalLight { illuminance: light_consts::lux::OVERCAST_DAY,
                       shadow_maps_enabled: true, ..default() },
    Transform::from_rotation(Quat::from_rotation_x(-PI / 4.)),
    // Defaults suit huge scenes; tighten for quality:
    CascadeShadowConfigBuilder { first_cascade_far_bound: 4.0, maximum_distance: 10.0, ..default() }.build(),
));
commands.insert_resource(GlobalAmbientLight { color: ORANGE_RED.into(), brightness: 200.0, ..default() });
```

### Shadow opt-out & alpha-mask shadows
`NotShadowCaster` / `NotShadowReceiver` components (`bevy::light::*`) per mesh — use for billboards, sky meshes, view models. `AlphaMode::Mask` materials cast correctly-cutout shadows. `examples/3d/shadow_caster_receiver.rs`, `examples/3d/lighting.rs`

### Exposure
`Exposure::from_physical_camera(PhysicalCameraParameters { aperture_f_stops, shutter_speed_s, sensitivity_iso, sensor_height })` on the camera entity. `examples/3d/lighting.rs`

---

## Fog & Skybox

### Distance fog — component on the camera
`examples/3d/fog.rs`, `examples/3d/atmospheric_fog.rs`

```rust
commands.spawn((
    Camera3d::default(),
    DistanceFog {
        color: Color::srgba(0.35, 0.48, 0.66, 1.0),
        directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5), // sun glow through fog
        directional_light_exponent: 30.0,
        falloff: FogFalloff::from_visibility_colors(
            15.0,                          // world units of ≥5%-contrast visibility
            Color::srgb(0.35, 0.5, 0.66),  // extinction
            Color::srgb(0.8, 0.844, 1.0),  // inscattering
        ),
    },
));
// Alternatives: FogFalloff::Linear { start, end }, FogFalloff::Exponential { density }
```

Toggle fog cheaply by driving `fog.color.set_alpha(..)`.

### Skybox
Cubemap component on the camera; PNG strips need reinterpretation after load. `examples/3d/skybox.rs`

```rust
commands.spawn((Camera3d::default(), Skybox { image: Some(handle.clone()), brightness: 1000.0, ..default() }));

// once loaded, if it's a vertical strip PNG:
if image.texture_descriptor.array_layer_count() == 1 {
    let layers = image.height() / image.width();
    image.reinterpret_stacked_2d_as_array(layers).unwrap();
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube), ..default() });
}
```

---

## Cameras

### Multiple cameras, order, clear color
`Camera { order }` sets render sequence; later cameras draw on top. Use `ClearColorConfig::None` on overlay cameras. `examples/camera/2d_on_ui.rs`

```rust
commands.spawn((Camera2d, IsDefaultUiCamera));               // UI target by default
commands.spawn((
    Camera2d,
    Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
    RenderLayers::layer(1),                                   // only draws layer-1 entities
));
```

This is also the 2D-HUD-over-3D-world pattern: a `Camera3d` (order 0) plus a `Camera2d`/UI camera (order 1) with `ClearColorConfig::None`.

### Orthographic 3D (isometric)
Always build via `Projection::from(..)` and `OrthographicProjection::default_3d()` (the 2D default has wrong near/far). `examples/3d/orthographic.rs`

```rust
commands.spawn((
    Camera3d::default(),
    Projection::from(OrthographicProjection {
        scaling_mode: ScalingMode::FixedVertical { viewport_height: 6.0 },
        ..OrthographicProjection::default_3d()
    }),
    Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
));
```

### Zoom
Match on the `Projection` enum: orthographic zooms via `scale` (multiplicative feels right), perspective via `fov` clamp. `examples/camera/projection_zoom.rs`

```rust
match *projection {
    Projection::Orthographic(ref mut o) => o.scale = (o.scale * (1. + delta)).clamp(0.1, 10.0),
    Projection::Perspective(ref mut p) => p.fov = (p.fov + delta).clamp(min, max),
    _ => (),
}
```

### Orbit camera
Yaw/pitch from `AccumulatedMouseMotion` (do NOT multiply mouse deltas by delta-time), clamp pitch, then place camera behind target. `examples/camera/camera_orbit.rs`

```rust
let (yaw, pitch, roll) = camera.rotation.to_euler(EulerRot::YXZ);
let pitch = (pitch + delta.y * pitch_speed).clamp(-limit, limit);
camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw + delta.x * yaw_speed, pitch, roll);
camera.translation = target - camera.forward() * orbit_distance;
```

### Split screen / viewports
`Camera.viewport: Some(Viewport { physical_position, physical_size, .. })`; resize on `MessageReader<WindowResized>` (fires on creation too). Per-viewport UI: root node with `UiTargetCamera(camera_entity)`; find a button's camera via `ComputedUiTargetCamera`. `examples/3d/split_screen.rs`, `examples/2d/2d_viewport_to_world.rs`

### Sub views (zoom/magnify/screen-shake without moving the camera)
`Camera { sub_camera_view: Some(SubCameraView { full_size, offset, size }), .. }` — ratios matter, not absolute pixels. Works for perspective and orthographic. `examples/3d/camera_sub_view.rs`

### Custom projection
Implement `CameraProjection` (`get_clip_from_view`, `get_clip_from_view_for_sub`, `update`, `far`, `get_frustum_corners`), attach with `Projection::custom(my_projection)`. `examples/camera/custom_projection.rs`

### Render to texture
`examples/3d/render_to_texture.rs`, `examples/2d/pixel_grid_snap.rs`

```rust
let image = Image::new_target_texture(512, 512, TextureFormat::Rgba8Unorm,
                                      Some(TextureFormat::Rgba8UnormSrgb));
let handle = images.add(image);
commands.spawn((
    Camera3d::default(),
    Camera { order: -1, clear_color: Color::WHITE.into(), ..default() },
    RenderTarget::Image(handle.clone().into()),   // separate component in 0.19
    RenderLayers::layer(1),
));
// use `handle` as base_color_texture / Sprite image / UI image
```

Pixel-perfect low-res pipeline: low-res camera renders layer 0 to a canvas image; a second camera on layer 1 draws the canvas `Sprite`, integer-scaled to the window via `OrthographicProjection.scale = 1. / scale.round()`. `examples/2d/pixel_grid_snap.rs`

### Cursor → world
`examples/2d/2d_viewport_to_world.rs`, `examples/3d/3d_viewport_to_world.rs`

```rust
// 2D: direct point
let world_pos = camera.viewport_to_world_2d(cam_transform, cursor_pos)?;
// 3D: ray + plane intersection (ground picking)
let ray = camera.viewport_to_world(cam_transform, cursor_pos)?;
let point = ray.plane_intersection_point(ground.translation(), InfinitePlane3d::new(ground.up()));
// inverse: camera.world_to_viewport(cam_transform, world_pos)
```

Run cursor-follow drawing in `PostUpdate` `.after(TransformSystems::Propagate)` to avoid one-frame lag.

### Mesh raycasting
`MeshRayCast` system param — no plugin needed for manual casts; `Res<RayMap>` holds pointer rays from picking. `examples/3d/mesh_ray_cast.rs`

```rust
fn cast(mut ray_cast: MeshRayCast, ray_map: Res<RayMap>) {
    for (_, ray) in ray_map.iter() {
        if let Some((entity, hit)) = ray_cast.cast_ray(*ray, &MeshRayCastSettings::default()).first() {
            // hit.point, hit.normal
        }
    }
}
```

---

## Render Layers

`RenderLayers` (`bevy::camera::visibility::RenderLayers`) filters which cameras draw which entities. No component = layer 0. **Lights must share a layer with what they illuminate** — add them to every relevant layer. `examples/3d/render_to_texture.rs`, `examples/camera/first_person_view_model.rs`

```rust
RenderLayers::layer(1)
RenderLayers::layer(0).with(1)
RenderLayers::from_layers(&[0, 1])
```

First-person view-model pattern (applies to any "always on top" 3D overlay): two `Camera3d` children of one rig — world camera (layer 0, order 0) and overlay camera (layer 1, order 1) — overlay meshes get `RenderLayers::layer(1)` + `NotShadowCaster`.

---

## Post-processing on the camera

Bloom + tonemapping are per-camera components. `examples/2d/bloom_2d.rs`, `examples/3d/bloom_3d.rs`, `examples/3d/tonemapping.rs`

```rust
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::post_process::bloom::Bloom;

commands.spawn((
    Camera2d, // or Camera3d::default()
    Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
    Tonemapping::TonyMcMapface, // desaturates-to-white; best with bloom
    Bloom::default(),           // fields: intensity, prefilter.threshold, composite_mode...
    DebandDither::Enabled,
));
// Emissive colors > 1.0 (e.g. Color::srgb(5.0, 5.0, 5.0)) are what actually bloom.
// Toggle: commands.entity(cam).remove::<Bloom>() / .insert(Bloom::default()).
```

Tonemapping variants: `None, AcesFitted, AgX, BlenderFilmic, Reinhard, ReinhardLuminance, SomewhatBoringDisplayTransform, TonyMcMapface, KhronosPbrNeutral`. HDR output: add the `Hdr` component (`bevy::camera::Hdr`). `examples/3d/blend_modes.rs`

---

## Transforms

### Core ops
`examples/transforms/transform.rs`, `examples/transforms/3d_rotation.rs`, `examples/3d/parenting.rs`

```rust
transform.rotate_y(speed * TAU * time.delta_secs());       // rotate around local origin
transform.translation += transform.forward() * speed * dt;  // forward() is -Z, returns Dir3
transform.rotate_around(Vec3::ZERO, Quat::from_rotation_y(angle)); // orbit a point
transform.look_to(Vec3::NEG_Z, Vec3::Y);
let target = transform.looking_at(point, *transform.local_y()); // non-mutating
transform.rotation = transform.rotation.lerp(target.rotation, t); // smooth turn
```

- Parent-child: `children![(...)]` — child `Transform` is relative; propagation is automatic (`GlobalTransform` updated in `PostUpdate`; order after it with `.after(TransformSystems::Propagate)`).
- Smooth approach: `rotation.smooth_nudge(&target_rotation, 3.0, time.delta_secs())` (`bevy::math::StableInterpolate`). `examples/transforms/align.rs`

### Two-axis alignment
`Transform::align`/`aligned_by`: primary axis matched exactly, secondary as close as possible — e.g. face a direction while staying upright. `examples/transforms/align.rs`

```rust
let t = Transform::IDENTITY.aligned_by(Vec3::NEG_Z, main_dir, Vec3::X, secondary_dir);
```

---

## Gizmos

### Immediate mode — `Gizmos` system param
Draw every frame from any system; nothing is retained. `examples/gizmos/2d_gizmos.rs`, `examples/gizmos/3d_gizmos.rs`

```rust
fn draw(mut gizmos: Gizmos) {
    // 2D (Isometry2d / Rot2 based)
    gizmos.line_2d(a, b, RED);
    gizmos.rect_2d(Isometry2d::IDENTITY, Vec2::splat(650.), BLACK);
    gizmos.circle_2d(pos, 10., WHITE);                     // .resolution(64) for big circles
    gizmos.arrow_2d(Vec2::ZERO, dir * 50., GREEN).with_double_end().with_tip_length(10.);
    gizmos.grid_2d(Isometry2d::IDENTITY, UVec2::new(16, 9), Vec2::splat(80.), LinearRgba::gray(0.05));

    // 3D (Isometry3d based; many accept plain Vec3/Quat)
    gizmos.sphere(Vec3::splat(10.0), 1.0, PURPLE);
    gizmos.cube(Transform::from_translation(Vec3::Y * 0.5), BLACK);
    gizmos.ray(origin, direction_vec, BLUE);
    gizmos.arrow(Vec3::ZERO, Vec3::splat(1.5), YELLOW);
    gizmos.linestrip_gradient([(p0, RED), (p1, BLUE)]);
    gizmos.grid(Quat::from_rotation_x(PI / 2.), UVec2::splat(20), Vec2::splat(2.), LinearRgba::gray(0.65));
    gizmos.axes(transform, length);                        // RGB = XYZ axes of a Transform
    gizmos.primitive_3d(&Plane3d { normal: Dir3::Y, half_size: Vec2::ONE }, iso, GREEN);
}
```

### Text gizmos
`examples/gizmos/3d_text_gizmos.rs`, `examples/gizmos/anchored_text_gizmos.rs`

```rust
gizmos.text(Isometry3d::new(pos, rot), "label", 1.0, Vec2::ZERO, RED);        // 3D, size in world units
gizmos.text_2d(Isometry2d::from_translation(pos), "label", 25., anchor, RED); // anchor: (0,0)=center, (-0.5,0)=left edge
```

### Config groups & global settings
`examples/gizmos/2d_gizmos.rs`, `examples/gizmos/3d_gizmos.rs`

```rust
#[derive(Default, Reflect, GizmoConfigGroup)]
struct MyGizmos;
app.init_gizmo_group::<MyGizmos>();
fn draw(mut mine: Gizmos<MyGizmos>) { /* separate width/style/visibility */ }

fn tweak(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 5.;
    config.line.style = GizmoLineStyle::Dashed { gap_scale: 3.0, line_scale: 5.0 }; // or Dotted/Solid
    config.depth_bias = -1.;        // draw on top of scene geometry
    config.line.perspective = true; // width scales with distance
    config.enabled = false;
    // Debug AABBs everywhere: store.config_mut::<AabbGizmoConfigGroup>().1.draw_all = true;
    // (or per-entity ShowAabbGizmo component)
}
```

### Retained gizmos — `Gizmo` component
For many static lines, an asset-backed component beats the per-frame system param. `examples/gizmos/3d_gizmos.rs`

```rust
let mut g = GizmoAsset::new();
g.sphere(Isometry3d::IDENTITY, 0.5, CRIMSON).resolution(10_000);
commands.spawn((
    Gizmo { handle: gizmo_assets.add(g),
            line_config: GizmoLineConfig { width: 5., ..default() }, ..default() },
    Transform::from_xyz(4., 1., 0.),
));
```

### Light gizmos & transform gizmo
- `LightGizmoConfigGroup` visualizes point/spot/directional light extents; color modes `LightGizmoColor::{Manual, Varied, MatchLightColor, ByLightType}`. `examples/gizmos/light_gizmos.rs`
- Built-in editor-style manipulator: `TransformGizmoPlugin` + `MeshPickingPlugin`, mark camera `TransformGizmoCamera`, select via picking, modes in `TransformGizmoSettings` (`bevy::gizmos::transform_gizmo`). `examples/gizmos/transform_gizmo.rs`

---

## Quick pitfalls recap

- 2D draw order = Z translation for `Blend`; opaque/mask 2D meshes really use the depth buffer.
- Orthographic 3D must use `OrthographicProjection::default_3d()`.
- Mutate `Projection` through the enum (`Projection::Orthographic(ref mut o)`), not a concrete component.
- Lights need `RenderLayers` membership per layer they light; forgetting leaves render-to-texture passes unlit.
- Mouse motion deltas: don't scale by delta-time; held buttons/keys: do.
- Meshes you mutate at runtime need `RenderAssetUsages::MAIN_WORLD` kept at creation.
- `WindowResized` fires at startup — one resize-handler system covers initial viewport setup too.
