# Audio, Input & Windowing (Bevy 0.19)

Distilled from the Bevy 0.19 examples tree. Note: 0.19 renamed buffered Events to **Messages** — use `MessageReader<T>` / `MessageWriter<T>`, never `EventReader`.

---

## Audio (built-in `bevy_audio`)

> **Lifthrasir uses `bevy_kira_audio`, not the built-in audio.** This section is reference-only for reading upstream examples; do not introduce `AudioPlayer`/`AudioSink` into the project — use the kira APIs from the `lifthrasir-bevy-plugins` skill instead.

### Playback

Audio is entity-driven: spawn an `AudioPlayer` component with a `Handle<AudioSource>`. `PlaybackSettings` presets: `ONCE`, `LOOP`, `DESPAWN` (despawn entity when done — the fire-and-forget SFX pattern), `REMOVE`.

```rust
commands.spawn(AudioPlayer::new(asset_server.load("sounds/music.ogg")));
commands.spawn((AudioPlayer::new(sfx.clone()), PlaybackSettings::DESPAWN));
```

`examples/audio/audio.rs`, `examples/audio/play_sound_effect.rs`

### Control

Once playing, Bevy inserts an `AudioSink` component; query it to control playback.

```rust
fn pause(keys: Res<ButtonInput<KeyCode>>, mut sink: Single<&mut AudioSink, With<MyMusic>>) {
    if keys.just_pressed(KeyCode::Space) { sink.toggle_playback(); }
    // also: sink.toggle_mute(), sink.set_speed(1.5), sink.position(),
    // sink.set_volume(sink.volume().increase_by_percentage(10.0))
}
```

Crossfades: spawn new track with `volume: Volume::SILENT`, then lerp with `Volume::SILENT.fade_towards(Volume::Linear(1.0), t)` on the sink (`examples/audio/soundtrack.rs`).

`examples/audio/audio_control.rs`

### Spatial

Emitter: `PlaybackSettings::LOOP.with_spatial(true)` alongside the `AudioPlayer`. Listener: a `SpatialListener::new(ear_gap)` component on a transform entity. Control via `SpatialAudioSink`. Panning follows relative transforms automatically. `examples/audio/spatial_audio_3d.rs`, `spatial_audio_2d.rs`

### Custom sources (notable only)

Procedural audio: implement `Decodable` (decoder is an `Iterator<Item = f32>` + `rodio::Source`) on an `Asset` type, register with `app.add_audio_source::<SineAudio>()`, then play via `AudioPlayer(handle)`. Global volume: `AudioPlugin { global_volume: Volume::Linear(0.2).into(), .. }`. `examples/audio/decodable.rs`, `pitch.rs`

---

## Input

### Keyboard — polling with `ButtonInput`

Two resources: `ButtonInput<KeyCode>` (physical key location, layout-independent — use for game hotkeys) and `ButtonInput<Key>` (logical key — use when the symbol matters, e.g. `?`, `+`/`-`).

```rust
fn input(keys: Res<ButtonInput<KeyCode>>, logical: Res<ButtonInput<Key>>) {
    if keys.just_pressed(KeyCode::KeyA) { /* edge-triggered, once */ }
    if keys.pressed(KeyCode::KeyA) { /* level-triggered, every frame */ }
    if keys.just_released(KeyCode::KeyA) { /* release edge */ }
    if logical.just_pressed(Key::Character("?".into())) { /* layout-aware */ }
}
```

`examples/input/keyboard_input.rs`

### Modifiers

No dedicated modifier API — combine `any_pressed`:

```rust
let shift = input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
let ctrl = input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
if ctrl && shift && input.just_pressed(KeyCode::KeyA) { /* Ctrl+Shift+A */ }
```

`examples/input/keyboard_modifiers.rs`

### Text input (chat / slash commands)

For typed text (chat box), read `KeyboardInput` messages and extract `Key::Character` from `logical_key` — this respects layout, shift, and dead keys, unlike `KeyCode`:

```rust
fn chat_typing(mut keyboard: MessageReader<KeyboardInput>) {
    for ev in keyboard.read() {
        if !ev.state.is_pressed() { continue; }
        match &ev.logical_key {
            Key::Character(c) => { /* append c (a SmolStr) to the chat buffer */ }
            Key::Backspace => { /* pop */ }
            Key::Enter => { /* submit */ }
            _ => {}
        }
    }
}
```

Raw event stream (`KeyboardInput` has `key_code`, `logical_key`, `state`, `repeat`, `window`) — `examples/input/keyboard_input_events.rs`, `examples/input/char_input_events.rs`.

**IME** (CJK composition): set `window.ime_enabled = true` (and `ime_position`) while a text field is focused, then read `MessageReader<Ime>` — `Ime::Preedit { value, cursor, .. }` for the in-progress composition, `Ime::Commit { value, .. }` for finalized text. Defined in `crates/bevy_window/src/event.rs`; no dedicated example in 0.19.

### Mouse

Buttons poll like keyboard. For motion/scroll, prefer the pre-summed per-frame resources over draining message readers:

```rust
fn mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,     // Vec2 delta this frame
    scroll: Res<AccumulatedMouseScroll>,     // Vec2 delta this frame (camera zoom)
) {
    if buttons.just_pressed(MouseButton::Left) { /* click */ }
    if scroll.delta.y != 0.0 { /* zoom by scroll.delta.y */ }
}
```

Raw messages when you need per-event data: `MessageReader<MouseButtonInput>`, `MessageReader<MouseMotion>`, `MessageReader<MouseWheel>` (has `unit: MouseScrollUnit::{Line, Pixel}`), `MessageReader<CursorMoved>` (window-space position). macOS-only gestures: `PinchGesture`, `RotationGesture`, `DoubleTapGesture`.

`examples/input/mouse_input.rs`, `examples/input/mouse_input_events.rs`

### Cursor grab

`CursorOptions` is its own component on the window entity (not a `Window` field in 0.19):

```rust
fn grab(mut cursor: Single<&mut CursorOptions>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Escape) {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None; // or Locked / Confined
    }
}
```

`CursorGrabMode::Locked` pins the cursor in place (mouselook); `Confined` keeps it inside the window. Platform support varies. `examples/input/mouse_grab.rs`

### Gamepad (brief)

Gamepads are entities with a `Gamepad` component; poll it directly:

```rust
fn pads(gamepads: Query<(Entity, &Gamepad)>) {
    for (_e, gamepad) in &gamepads {
        if gamepad.just_pressed(GamepadButton::South) { /* A/Cross */ }
        let x = gamepad.get(GamepadAxis::LeftStickX).unwrap(); // f32, deadzone yourself
    }
}
```

`examples/input/gamepad_input.rs` (events: `gamepad_input_events.rs`, rumble: `gamepad_rumble.rs`)

---

## Windows

The primary window is an entity with a `Window` component; mutate it at runtime via `Single<&mut Window>`. Configure at startup through `DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { .. }), .. })`.

### Settings (mode, resolution, decorations, vsync)

```rust
Window {
    title: "Lifthrasir".into(),
    resolution: (1280, 720).into(),          // or WindowResolution::new(w, h)
    present_mode: PresentMode::AutoVsync,    // AutoNoVsync to uncap
    decorations: true,
    window_theme: Some(WindowTheme::Dark),
    enabled_buttons: EnabledButtons { maximize: false, ..Default::default() },
    visible: false,  // spawn hidden, set window.visible = true after ~3 frames to avoid the white flash
    ..default()
}
```

Runtime: `window.present_mode = ...`, `window.window_level = WindowLevel::AlwaysOnTop`, `window.resolution.set(w, h)`. React to resizes with `MessageReader<WindowResized>`. **Fullscreen toggle**: `window.mode = WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)` or back to `WindowMode::Windowed` (borderless variant: `WindowMode::BorderlessFullscreen(MonitorSelection::Current)`).

`examples/window/window_settings.rs`, `examples/window/window_resizing.rs`

### Multiple windows

Spawn another `Window` entity; render into it by putting `RenderTarget::Window(WindowRef::Entity(win))` on a camera (in 0.19 `RenderTarget` is a component from `bevy::camera`, not a `Camera` field). UI must pick its camera with `UiTargetCamera(camera_entity)`.

```rust
let win = commands.spawn(Window { title: "Second".into(), ..default() }).id();
commands.spawn((Camera3d::default(), RenderTarget::Window(WindowRef::Entity(win))));
```

`examples/window/multiple_windows.rs`

### Scale factor override

```rust
window.resolution.set_scale_factor_override(Some(1.0)); // None = OS default
let sf = window.scale_factor(); // effective factor
```

Useful for forcing 1:1 pixel UI regardless of OS DPI. `examples/window/scale_factor_override.rs`

### Transparent window

`transparent: true`, `decorations: false`, plus `ClearColor(Color::NONE)`. macOS needs `composite_alpha_mode: CompositeAlphaMode::PostMultiplied`, Linux `PreMultiplied`. Platform-dependent. `examples/window/transparent_window.rs`

### Low-power / desktop-app mode

`WinitSettings` resource controls the event loop: `WinitSettings::game()` (continuous, default) vs `WinitSettings::desktop_app()` (reactive — only updates on winit events). Custom mix:

```rust
app.insert_resource(WinitSettings {
    focused_mode: UpdateMode::Continuous,
    unfocused_mode: UpdateMode::reactive_low_power(Duration::from_millis(10)),
});
```

In reactive mode, force a frame with `MessageWriter<RequestRedraw>` (e.g. to keep UI animations running). `examples/window/low_power.rs`

### Screenshots

Spawn a `Screenshot` entity and observe the capture:

```rust
commands.spawn(Screenshot::primary_window()).observe(save_to_disk("shot.png"));
```

(`bevy::render::view::screenshot::{Screenshot, save_to_disk, Capturing}` — `Capturing` marker exists while saving.) `examples/window/screenshot.rs`

### Cursor icon (system + custom image)

`CursorIcon` is a component you insert on the window entity (not present by default):

```rust
commands.entity(window_entity).insert(CursorIcon::from(SystemCursorIcon::Pointer));
// Custom image (requires "custom_cursor" feature; supports TextureAtlas animation, flip, rect):
commands.entity(window_entity).insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
    handle: asset_server.load("cursors/crosshair.png"),
    hotspot: (0, 0),
    ..Default::default()
})));
```

Relevant for RO-style cursor sprites. `examples/window/custom_cursor_image.rs`, `examples/window/window_settings.rs`

### Monitor info

Monitors are entities with a `Monitor` component (`name`, `physical_position`, `physical_width/height`, `refresh_rate_millihertz`, `scale_factor`); detect hotplug with `Added<Monitor>` / `RemovedComponents<Monitor>`. Windows carry `OnMonitor(Entity)`. Target one: `MonitorSelection::Entity(e)` in `WindowMode::Fullscreen` or `WindowPosition::Centered`. `examples/window/monitor_info.rs`

### Drag-move / drag-resize (undecorated windows)

For borderless windows, hand the drag to the OS — but **only** during a frame where left mouse was just pressed (winit panics otherwise):

```rust
if input.just_pressed(MouseButton::Left) {
    window.start_drag_move();                       // OS-native window drag
    // or: window.start_drag_resize(CompassOctant::SouthEast);
}
```

Note this moves the OS window itself — in-game draggable UI panels should keep using bevy_picking drag observers instead. `examples/window/window_drag_move.rs`

### Persisting window settings (0.19 addition)

`bevy::settings::SettingsPlugin` + a `#[derive(SettingsGroup)]` resource persists position/size/fullscreen across runs; save on `WindowCloseRequested` with `ExitCondition::DontExit` to intercept exit. Lifthrasir already has `bevy-persistent` for this role. `examples/window/persisting_window_settings.rs`
