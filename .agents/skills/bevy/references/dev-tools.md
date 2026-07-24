# Dev Tools, Diagnostics, BRP, Async & Math (Bevy 0.19)

Distilled from the Bevy 0.19 examples tree. Source paths are relative to the Bevy repo.

## FPS Overlay (`bevy_dev_tools`)

`FpsOverlayPlugin` (behind the `bevy_dev_tools` cargo feature) draws an FPS counter plus an optional frame-time graph. All settings live in the `FpsOverlayConfig` resource and are hot-mutable at runtime — toggling `enabled` is the idiomatic dev-tools on/off switch. Requires any camera to be present.

```rust
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};

app.add_plugins(FpsOverlayPlugin {
    config: FpsOverlayConfig {
        text_config: TextFont { font_size: FontSize::Px(42.0), ..default() },
        text_color: Color::srgb(0.0, 1.0, 0.0),
        refresh_interval: core::time::Duration::from_millis(100),
        enabled: true,
        frame_time_graph_config: FrameTimeGraphConfig {
            enabled: true, min_fps: 30.0, target_fps: 144.0,
        },
    },
});

fn toggle(input: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<FpsOverlayConfig>) {
    if input.just_pressed(KeyCode::F3) { overlay.enabled = !overlay.enabled; }
}
```

Note: font size is only numerically adjustable when `text_config.font_size` is the `FontSize::Px` variant.

Source: `examples/dev_tools/fps_overlay.rs`

## Built-in Diagnostics Plugins

Add after `DefaultPlugins` (they need the time plugin). `LogDiagnosticsPlugin` is just the console printer — the collector plugins work without it (e.g. feeding a custom overlay).

```rust
use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    LogDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};

app.add_plugins((
    LogDiagnosticsPlugin::default(),          // prints to console
    FrameTimeDiagnosticsPlugin::default(),    // FPS, FRAME_TIME, FRAME_COUNT
    EntityCountDiagnosticsPlugin::default(),  // ENTITY_COUNT
    SystemInformationDiagnosticsPlugin,       // PROCESS/SYSTEM_CPU_USAGE, *_MEM_USAGE
    bevy::render::diagnostic::RenderDiagnosticsPlugin, // render-app diagnostics (verbose)
));
```

Well-known paths are consts: `FrameTimeDiagnosticsPlugin::FPS`, `EntityCountDiagnosticsPlugin::ENTITY_COUNT`, `SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE`, etc.

Source: `examples/diagnostics/log_diagnostics.rs`

## Custom Diagnostics

Register a `Diagnostic` under a unique `DiagnosticPath`, then push measurements from any system via the `Diagnostics` system param (closure is lazy — only evaluated if the diagnostic is enabled).

```rust
use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};

const SYSTEM_ITERATION_COUNT: DiagnosticPath = DiagnosticPath::const_new("system_iteration_count");

app.register_diagnostic(Diagnostic::new(SYSTEM_ITERATION_COUNT).with_suffix(" iterations"));

fn my_system(mut diagnostics: Diagnostics) {
    diagnostics.add_measurement(&SYSTEM_ITERATION_COUNT, || 10.0);
}
```

Source: `examples/diagnostics/custom_diagnostic.rs`

## Reading & Toggling Diagnostics: `DiagnosticsStore`

`DiagnosticsStore` is the resource holding all registered diagnostics. Read values with `store.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed())`; disable collection at runtime by flipping the public `is_enabled` field.

```rust
fn toggle(mut store: ResMut<DiagnosticsStore>) {
    for diag in store.iter_mut() {
        diag.is_enabled = !diag.is_enabled;
    }
}
```

Source: `examples/diagnostics/enabling_disabling_diagnostic.rs`

## Log Filtering (`LogDiagnosticsState`) and log-once

`LogDiagnosticsPlugin` prints everything by default; the `LogDiagnosticsState` resource filters which `DiagnosticPath`s get logged:

```rust
fn setup_filtering(mut log_state: ResMut<LogDiagnosticsState>) {
    log_state.enable_filtering();                              // start filtering (empty = log nothing)
    log_state.extend_filter([FrameTimeDiagnosticsPlugin::FPS]); // allow these paths
    log_state.remove_filter(&FrameTimeDiagnosticsPlugin::FPS);  // drop one again
    log_state.disable_filtering();                             // back to log-everything
}
```

For per-callsite log spam control in ordinary systems, use the once variants: `info_once!`, `warn_once!`, `error_once!`, `debug_once!` (log once per callsite), or the generic `once!(expr)`.

Source: `examples/diagnostics/log_diagnostics.rs`

## BRP Server Setup (`bevy_remote` feature)

Two plugins: `RemotePlugin` provides the JSON-RPC method set; `RemoteHttpPlugin` serves it over HTTP (default `127.0.0.1:15702`; the render app gets its own `DEFAULT_RENDER_PORT`). Components/resources must be `Reflect` + reflected `Serialize`/`Deserialize` to round-trip over BRP.

```rust
use bevy::remote::{http::RemoteHttpPlugin, RemotePlugin};

app.add_plugins(RemotePlugin::default())
   .add_plugins(RemoteHttpPlugin::default());

#[derive(Component, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
struct Cube(f32);
```

Source: `examples/remote/server.rs`

## BRP Client Requests

Requests are JSON-RPC posted to the HTTP endpoint; Bevy ships typed param structs so a Rust client never handcrafts JSON shapes. Components are addressed by full type path (`std::any::type_name::<T>()`).

```rust
use bevy::remote::{builtin_methods::*, BrpRequest};

let req = BrpRequest {
    method: BRP_QUERY_METHOD.to_string(),   // "world.query"
    id: Some(serde_json::to_value(1)?),
    params: Some(serde_json::to_value(BrpQueryParams {
        data: BrpQuery {
            components: vec![type_name::<Transform>().to_string()],
            option: ComponentSelector::default(),  // or ComponentSelector::All
            has: Vec::default(),
        },
        strict: false,
        filter: BrpQueryFilter {
            without: vec![type_name::<ChildOf>().to_string()], // root entities only
            with: Vec::default(),
        },
    })?),
};
let res: serde_json::Value = ureq::post(&url).send_json(req)?.body_mut().read_json()?;
```

`BRP_WRITE_MESSAGE_METHOD` injects a Bevy `Message` remotely — e.g. writing `bevy_app::app::AppExit` with value `"Success"` shuts the app down.

Source: `examples/remote/client.rs`, `examples/remote/server.rs`

## BRP Integration Testing (drive the app remotely)

`examples/remote/integration_test.rs` (against `app_under_test.rs`) shows a full remote-driven UI test — the same techniques brp_extras wraps:

- **Screenshot**: `BRP_SPAWN_ENTITY_METHOD` spawns an entity with a `Screenshot` component (`{"Window": "Primary"}`), then `BRP_OBSERVE_METHOD` streams `ScreenshotCaptured` events as SSE (`data: ` lines); the event payload deserializes into `bevy::image::Image`. Window must be visible or the capture is black.
- **Locate UI**: query `UiGlobalTransform` filtered `with: [Button]`; it serializes as a flat `Affine2` array where indices 4/5 are the node center in *physical* pixels. Divide by `Window.resolution.scale_factor` for logical coords.
- **Synthetic input**: write `WindowEvent` messages — `CursorMoved` (positions picking), then `MouseButtonInput` `Pressed` + `Released` (picking needs both to emit `Pointer<Click>`).

Source: `examples/remote/integration_test.rs`, `examples/remote/app_under_test.rs`

## Async Compute: Task-Polling Pattern

Spawn work on `AsyncComputeTaskPool`, store the `Task` in a component, and poll each frame with `bevy::tasks::futures::check_ready`. **0.19 change**: do NOT use `block_on(poll_once(...))` — the example explicitly warns it is expensive, can block the main thread, and leaves a `Task<T>` that panics if awaited again. `check_ready` is the non-blocking replacement.

Returning a `CommandQueue` from the task lets the async side stage arbitrary world mutations (via `SystemState` inside a pushed `FnOnce(&mut World)`) that are applied deferred:

```rust
use bevy::{ecs::world::CommandQueue, tasks::{futures::check_ready, AsyncComputeTaskPool, Task}};

#[derive(Component)]
struct ComputeTransform(Task<CommandQueue>);

fn spawn_tasks(mut commands: Commands) {
    let pool = AsyncComputeTaskPool::get();
    let entity = commands.spawn_empty().id();
    let task = pool.spawn(async move {
        let mut queue = CommandQueue::default();
        queue.push(move |world: &mut World| {
            world.entity_mut(entity).insert(Transform::default());
        });
        queue
    });
    commands.entity(entity).insert(ComputeTransform(task));
}

fn handle_tasks(mut commands: Commands, mut tasks: Query<(Entity, &mut ComputeTransform)>) {
    for (entity, mut task) in &mut tasks {
        if let Some(mut queue) = check_ready(&mut task.0) {
            commands.append(&mut queue);
            commands.entity(entity).remove::<ComputeTransform>();
        }
    }
}
```

Source: `examples/async_tasks/async_compute.rs`

## Async Compute: Channel Pattern (detached tasks)

Alternative with no per-task polling: `.detach()` the task and send results over a `crossbeam_channel` stored in a resource; a system drains `receiver.try_iter()` each frame. Simpler when you don't need cancellation or per-task tracking.

```rust
#[derive(Resource)]
struct CubeChannel { sender: Sender<CubeFinished>, receiver: Receiver<CubeFinished> }

fn spawn_tasks(channel: Res<CubeChannel>) {
    let sender = channel.sender.clone();
    AsyncComputeTaskPool::get().spawn(async move {
        // ... await work ...
        let _ = sender.send(CubeFinished { /* result */ });
    }).detach();
}

fn handle_finished(channel: Res<CubeChannel>, mut commands: Commands) {
    for msg in channel.receiver.try_iter() { /* spawn/apply */ }
}
```

Source: `examples/async_tasks/async_channel_pattern.rs`

## External Thread Source

For an infinite external producer (network stream, file watcher), spawn a plain `std::thread` and bridge with a bounded crossbeam channel (std `Receiver` is `!Sync`, so crossbeam is required for the resource). Drain into ECS `Message`s — the example reads in `FixedUpdate` to decouple rates.

```rust
#[derive(Resource, Deref)]
struct StreamReceiver(Receiver<u32>);
#[derive(Message)]
struct StreamMessage(u32);

let (tx, rx) = crossbeam_channel::bounded::<u32>(1);
std::thread::spawn(move || loop { tx.send(produce()).unwrap(); }); // blocks until read
commands.insert_resource(StreamReceiver(rx));

fn read_stream(receiver: Res<StreamReceiver>, mut messages: MessageWriter<StreamMessage>) {
    for v in receiver.try_iter() { messages.write(StreamMessage(v)); }
}
```

Source: `examples/async_tasks/external_source_external_thread.rs`

## Bounding Volumes & Intersection Tests (`bevy::math::bounding`)

Any primitive (`Circle`, `Rectangle`, `Triangle2d`, `Segment2d`, `Capsule2d`, `RegularPolygon`, ...) produces a bound at a given `Isometry2d` (translation + `Rot2`):

```rust
use bevy::math::{bounding::*, Isometry2d};

let iso = Isometry2d::new(translation_xy, Rot2::radians(z_angle));
let aabb: Aabb2d = rect.aabb_2d(iso);              // BoundedVolume trait
let circle: BoundingCircle = rect.bounding_circle(iso);

// Volume-vs-volume (works cross-type)
let hit: bool = aabb.intersects(&circle);

// Ray casts: Dir2 is a validated unit direction
let ray = Ray2d { origin, direction: Dir2::from_xy(1.0, 0.3).unwrap() };
let cast = RayCast2d::from_ray(ray, max_distance);
let toi: Option<f32> = cast.aabb_intersection_at(&aabb);   // or circle_intersection_at
let point = cast.ray.origin + *cast.ray.direction * toi.unwrap();

// Shape casts (swept volumes along a ray)
let aabb_cast = AabbCast2d { aabb: Aabb2d::new(Vec2::ZERO, Vec2::splat(15.)), ray: cast };
let toi = aabb_cast.aabb_collision_at(aabb);
let circle_cast = BoundingCircleCast { circle: BoundingCircle::new(Vec2::ZERO, 15.), ray: /*...*/ };
```

`Aabb2d::new(center, half_size)`; readback via `.center()` / `.half_size()`, `BoundingCircle::new(center, radius)` / `.radius()`. 3D mirrors all of this (`Aabb3d`, `BoundingSphere`, `RayCast3d`, `Dir3`). Also note the `Or<(Changed<A>, Changed<B>)>` query pattern used to recompute volumes only when shape/transform changed.

Source: `examples/math/bounding_2d.rs`

## Random Shape Sampling (`ShapeSample`)

Primitives implement `ShapeSample`: draw uniform points from the interior or surface with any `rand` RNG. Keep a seeded RNG in a resource for reproducibility. Samples are in the shape's local space — apply `transform.transform_point(sample)` (or parent the spawned entity) for world placement.

```rust
use bevy::math::prelude::*;   // brings ShapeSample into scope
use rand::distr::Distribution;

let shape = Cuboid::from_length(2.9);
let p: Vec3 = shape.sample_interior(&mut rng);
let q: Vec3 = shape.sample_boundary(&mut rng);

// Bulk sampling via rand Distributions
let pts: Vec<Vec3> = shape.interior_dist().sample_iter(&mut rng).take(100).collect();
let surf: Vec<Vec3> = shape.boundary_dist().sample_iter(&mut rng).take(100).collect();
```

Source: `examples/math/random_sampling.rs`

## Cubic Splines & Curve Sampling

Spline builders (`CubicHermite::new(points, tangents)`, `CubicCardinalSpline::new_catmull_rom(points)`, `CubicBSpline::new(points)`, `CubicBezier`) convert to a sampleable `CubicCurve<P>` via `.to_curve()` (fallible — returns `Result`) or `.to_curve_cyclic()` for closed loops.

```rust
use bevy::math::cubic_splines::*;

let curve: CubicCurve<Vec2> = CubicHermite::new(points, tangents).to_curve()?;
curve.position(t);                      // sample at t (per-segment parameterization)
let n = 100 * curve.segments().len();   // scale resolution with segment count
gizmos.linestrip(curve.iter_positions(n).map(|p| p.extend(0.0)), Color::WHITE);
```

`iter_positions(n)` yields evenly spaced parameter samples across the whole curve; `iter_velocities`/`iter_accelerations` exist for derivatives. `CubicCurve` also implements the general `Curve<P>` trait (`.sample(t)`, `.domain()`) for composition with the curve adaptor APIs.

Source: `examples/math/cubic_splines.rs`

## Rot2 / Dir2 Notes

- `Rot2::radians(angle)` is the 2D rotation type used by `Isometry2d`; extract a Z angle from a 3D `Quat` with `transform.rotation.to_euler(EulerRot::YXZ).2`.
- `Dir2` / `Dir3` are validated unit vectors: construct with `Dir2::from_xy(x, y).unwrap()` (fails on zero/non-finite) or `Dir2::new_unchecked(v)` when already normalized; deref (`*dir`) to get the `Vec2`. Ray types require them, making unnormalized-direction bugs unrepresentable.
- `Segment2d::from_direction_and_length(dir, len)` is a typical `Dir2` consumer.

Source: `examples/math/bounding_2d.rs`

## Not Covered (and why)

- `examples/dev_tools/infinite_grid.rs`, `schedule_data.rs` — editor-style niceties, not relevant to a running game client.
- `examples/tools/gamepad_viewer.rs`, `scene_viewer/` — standalone utility apps; no reusable API patterns beyond what's above.
- `examples/math/custom_primitives.rs`, `render_primitives.rs` — implementing your own primitive types; only needed when adding new shape types.
