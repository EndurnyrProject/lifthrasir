# Bevy UI & Picking (Bevy 0.19)

Distilled from the official Bevy examples repo. No bundles: spawn `Node`, `Text`, `ImageNode` etc. as plain components. Picking observers use `On<Pointer<E>>`.

## Setup fundamentals

UI needs a camera. With multiple cameras, mark the UI one, or target explicitly per-root.

```rust
commands.spawn((Camera2d, IsDefaultUiCamera));
// Or bind a UI tree to a specific camera (one render target per UI layout, no RenderLayers):
commands.spawn((Node { /* root */ ..default() }, UiTargetCamera(camera_entity)));
```

`Val` helper functions are idiomatic in 0.19: `px(10)`, `percent(100)`, `auto()`, `vh(20)`. They also build `UiRect`s: `px(10).all()`, `MARGIN.right()`, `auto().horizontal()`.
`examples/ui/ui_target_camera.rs`, `examples/ui/scroll_and_overflow/scroll.rs`

## Flex layout

Default `display` is flex. Root nodes typically fill the window with `percent(100)`.

```rust
Node {
    width: percent(100), height: percent(100),
    flex_direction: FlexDirection::Column,
    align_items: AlignItems::Center,        // cross axis
    justify_content: JustifyContent::Center, // main axis
    row_gap: px(12), column_gap: px(12),
    padding: UiRect::all(px(10)),
    flex_wrap: FlexWrap::Wrap,
    ..default()
}
```

Margins/padding cannot be applied to text nodes directly — wrap `Text` in a parent `Node`. Children can be declared inline with `children![...]`, `.with_children(|b| ...)`, `.with_child(bundle)`, or `Children::spawn(SpawnIter(iter))` for dynamic lists.
`examples/ui/layout/flex_layout.rs`, `examples/ui/styling/borders.rs`

## Grid layout

```rust
Node {
    display: Display::Grid,
    grid_template_columns: vec![GridTrack::min_content(), GridTrack::flex(1.0)],
    grid_template_rows: vec![GridTrack::auto(), GridTrack::flex(1.0), GridTrack::px(20.)],
    ..default()
}
// Evenly sized inventory-style grid:
grid_template_columns: RepeatedGridTrack::flex(4, 1.0),
grid_template_rows: RepeatedGridTrack::flex(4, 1.0),
// Item placement (omit for auto-flow):
Node { grid_column: GridPlacement::span(2), ..default() }           // span
Node { grid_row: GridPlacement::start(row + 1), ..default() }       // explicit 1-based cell
```

Grid items with `AlignItems::Stretch`/`JustifyItems::Stretch` (default) take size from their cell — no explicit size needed.
`examples/ui/layout/grid.rs`, `examples/ui/ui_drag_and_drop.rs`

## Absolute positioning and anchoring

```rust
Node { position_type: PositionType::Absolute, bottom: px(5), right: px(5), ..default() }
```

Anchor-style placement without absolute positioning uses `auto` margins inside a stretched cell:

```rust
Node { margin: UiRect::all(auto()), ..default() }          // centered
Node { right: px(10), margin: UiRect::vertical(auto()), ..default() } // right-center
```

A centered modal over content: absolute position + `margin: UiRect { left: auto(), right: auto(), .. }`.
`examples/ui/layout/anchor_layout.rs`, `examples/ui/layout/grid.rs`

## Display vs Visibility

- `Display::None` removes the node from layout (siblings reflow). Toggle for tab panels / hidden windows.
- `Visibility::Hidden` hides but **still occupies layout space**.

```rust
node.display = match node.display { Display::Flex => Display::None, Display::None => Display::Flex, _ => node.display };
```
`examples/ui/layout/display_and_visibility.rs`

## Z-index and stacking

- `ZIndex(i32)`: depth relative to **siblings** only.
- `GlobalZIndex(i32)`: depth against the entire UI, escapes the parent stacking context (e.g. lift a dragged window above everything).

```rust
commands.spawn((Node { .. }, ZIndex(2)));         // above siblings
commands.spawn((Node { .. }, GlobalZIndex(1)));   // above the whole UI, even its own parent
commands.spawn((Node { .. }, GlobalZIndex(-1)));  // below everything
```
`examples/ui/layout/z_index.rs`

## Overflow and clipping

```rust
Node { overflow: Overflow::clip(), ..default() }   // also clip_x() / clip_y() / visible()
// Control where clipping starts relative to the box:
Node {
    overflow: Overflow::clip(),
    overflow_clip_margin: OverflowClipMargin::content_box(), // border_box() / padding_box() / .with_margin(25.)
    ..default()
}
```
`examples/ui/scroll_and_overflow/overflow.rs`, `examples/ui/scroll_and_overflow/overflow_clip_margin.rs`

## Scrolling

Mark the container `Overflow::scroll()` (`scroll_x()`/`scroll_y()`), then write to its `ScrollPosition` component (logical px). Max scroll offset comes from `ComputedNode`:

```rust
Node { overflow: Overflow::scroll_y(), scrollbar_width: 20., ..default() }

let max_offset = (computed.content_size() - computed.size()) * computed.inverse_scale_factor();
scroll_position.y = (scroll_position.y + delta.y).clamp(0., max_offset.y);
```

The canonical mouse-wheel wiring converts `MouseWheel` messages into a custom propagating entity event triggered on hovered entities (from `HoverMap`), and each scrollable consumes the delta axis it can use, stopping propagation when fully consumed — this makes nested scroll areas behave:

```rust
#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct Scroll { entity: Entity, delta: Vec2 }

fn send_scroll_events(mut wheel: MessageReader<MouseWheel>, hover_map: Res<HoverMap>, mut commands: Commands) {
    for mw in wheel.read() {
        let mut delta = -Vec2::new(mw.x, mw.y);
        if mw.unit == MouseScrollUnit::Line { delta *= LINE_HEIGHT; }
        for map in hover_map.values() {
            for entity in map.keys().copied() { commands.trigger(Scroll { entity, delta }); }
        }
    }
}
// observer: fn on_scroll(mut scroll: On<Scroll>, ...) { ...; scroll.propagate(false); }
```

Sticky headers inside a scroll area: `IgnoreScroll(BVec2 { x, y })` + `ZIndex` + `BackgroundColor`.

Drag-to-scroll: capture `computed_node.scroll_position * computed_node.inverse_scale_factor` on `DragStart`, then on `Drag` set `scroll_position.0 = (start - drag.distance / ui_scale.0).max(Vec2::ZERO)`.

Built-in scrollbar widgets (`bevy::ui_widgets`): a `Scrollbar { orientation, target: scroll_area_entity, min_thumb_length }` node with a `ScrollbarThumb { border_radius, border }` child; style the thumb from `Hovered` / `ScrollbarDragState`.
`examples/ui/scroll_and_overflow/scroll.rs`, `drag_to_scroll.rs`, `scrollbars.rs`

## Text (UI)

```rust
commands.spawn((
    Text::new("hello\nbevy!"),
    TextFont { font: asset_server.load("fonts/FiraSans-Bold.ttf").into(), font_size: FontSize::Px(20.0), ..default() },
    TextColor(GOLD.into()),
    TextShadow::default(),
    TextLayout::justify(Justify::Center),   // multi-line alignment, not node position
    Node { position_type: PositionType::Absolute, bottom: px(5), right: px(5), ..default() },
));
```

Key facts:
- `TextFont.font` is a `FontSource` (`Handle<Font>::into()`); `font_size` is the `FontSize` enum (`Px`, `Vh`, ...), not a bare f32. `TextFont::from_font_size(14.0)` is the shorthand.
- Rich text = child entities with `TextSpan` (each with its own `TextFont`/`TextColor`). Update by writing through the deref: `**span = format!("{value:.2}")`.
- Wrapping: constrain width via `Node { max_width: px(300), .. }`; control breaking with `TextLayout { linebreak: LineBreak::WordBoundary | AnyCharacter | WordOrCharacter | NoWrap, ..default() }`.
- `Underline` / strikethrough components exist; OpenType features via `FontFeatures::builder().enable(FontFeatureTag::SMALL_CAPS).build()`.
- **UI `Text` vs `Text2d`**: `Text` lives in the UI layout tree (needs a `Node` context, screen-space). `Text2d` (+ `Text2dShadow`, `Anchor`) is a world-space 2D entity positioned with `Transform` — use it for in-world labels, not HUD.

`examples/ui/text/text.rs`, `text_debug.rs`, `text_wrap_debug.rs`, `examples/2d/text2d.rs`

## Borders, radius, outline, shadow

```rust
(
    Node { border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), ..default() },
    BorderColor::all(Color::BLACK),          // or per-side: BorderColor { top, bottom, left, right }
    Outline { width: px(2), offset: px(2), color: Color::WHITE }, // drawn outside, no layout impact
    BoxShadow::new(color, x_offset, y_offset, spread, blur),      // supports multiple shadows
)
```

`border_radius` is a **field on `Node`** in 0.19 (not a separate component); `BorderRadius::MAX` makes pills/circles. `Outline` color set to `Color::NONE` hides it — cheap hover highlight (`outline.color = RED.into()` on `Interaction`/`Over`). `border_color.set_all(c)` mutates all sides.
`examples/ui/styling/borders.rs`, `box_shadow.rs`, `examples/ui/scroll_and_overflow/overflow.rs`

## UI images, atlases, 9-slice

```rust
ImageNode::new(image_handle)                                  // stretched into node size
ImageNode { image, image_mode: NodeImageMode::Stretch, flip_x: true, color: GOLD.into(), ..default() }

// Atlas (sprite-sheet icons); animate by mutating image_node.texture_atlas.index:
ImageNode::from_atlas_image(texture, TextureAtlas::from(layout_handle))

// 9-patch window chrome:
let slicer = TextureSlicer {
    border: BorderRect::all(22.0),
    center_scale_mode: SliceScaleMode::Stretch,   // or Tile { stretch_value }
    sides_scale_mode: SliceScaleMode::Stretch,
    max_corner_scale: 1.0,
};
ImageNode { image, image_mode: NodeImageMode::Sliced(slicer), ..default() }
```

`ImageNode.color` tints (usable for hover/press states). Atlas + slicing combine: `.with_mode(NodeImageMode::Sliced(...))` on an atlas image. Pixel art: `DefaultPlugins.set(ImagePlugin::default_nearest())`.
`examples/ui/images/image_node.rs`, `ui_texture_atlas.rs`, `ui_texture_slice.rs`, `ui_texture_slice_flip_and_tile.rs`, `ui_texture_atlas_slice.rs`

## UiTransform (translate / rotate / scale)

UI nodes ignore 3D `Transform`; visual transforms use `UiTransform` — post-layout, layout is unaffected:

```rust
UiTransform::from_rotation(Rot2::radians(FRAC_PI_2))
transform.rotation *= Rot2::radians(-FRAC_PI_8);
transform.scale = Vec2::splat(1.5);
transform.translation = Val2::px(x, y);      // Val-based, so percent works too
```
`examples/ui/ui_transform.rs`

## Viewport nodes and render-to-texture UI

Embed a camera's view inside the UI (character preview panes): render the camera to an `Image` (`RenderTarget::Image`, texture usages `TEXTURE_BINDING | COPY_DST | RENDER_ATTACHMENT`), then spawn a `ViewportNode::new(camera)` node. Picking passes through automatically via `bevy::ui::widget::viewport_picking` — observers on 3D objects seen through the viewport just work.

```rust
commands.spawn((
    Node { position_type: PositionType::Absolute, width: px(200), height: px(200), ..default() },
    ViewportNode::new(camera_entity),
));
```

The inverse (UI drawn onto a 3D surface): give the UI root `UiTargetCamera(texture_camera)` where that `Camera2d` renders to an image used as a material texture. Pointer input needs manual driving — raycast the mesh, take `hit.uv * texture_size`, and write `PointerInput` messages for a `PointerId::Custom(uuid)` pointer.
`examples/ui/widgets/viewport_node.rs`, `examples/ui/render_ui_to_texture.rs`

## World-to-viewport positioning (health bars)

Project a world position and drive an absolute-positioned node — the classic overhead label/health bar:

```rust
fn track(labels: Query<(&mut Node, &Label)>, camera: Single<(&Camera, &GlobalTransform)>, targets: Query<&GlobalTransform>) {
    let (camera, camera_transform) = *camera;
    for (mut node, label) in &mut labels {
        let world_pos = targets.get(label.entity).unwrap().translation() + Vec3::Y;
        if let Ok(viewport_pos) = camera.world_to_viewport(camera_transform, world_pos) {
            node.left = px(viewport_pos.x);
            node.top = px(viewport_pos.y);
        }
    }
}
```

Center the node on the point with a child layout or `UiTransform` translation of -50%.
`examples/3d/blend_modes.rs` (label tracking)

## RelativeCursorPosition

Insert `RelativeCursorPosition::default()` on a node to get the cursor position normalized to the node's rect ((0,0) top-left, (1,1) bottom-right) — good for sliders, color pickers, minimap clicks. Accounts for camera viewport offset.

```rust
if let Some(pos) = relative_cursor_position.normalized { /* Vec2 in [0,1] when inside */ }
relative_cursor_position.cursor_over()   // bool
```
`examples/ui/relative_cursor_position.rs`

## UiScale

Global UI multiplier resource: `commands.insert_resource(UiScale(1.25))`. Remember it (and window scale factor) when doing pixel math against `ComputedNode` — computed values are physical px; convert with `computed.inverse_scale_factor()`.
`examples/ui/ui_scaling.rs`, `examples/ui/scroll_and_overflow/drag_to_scroll.rs`

## Focus and tab navigation

`bevy::input_focus`: add `TabNavigationPlugin`, group focusables under a node with `TabGroup::new(n)` (or `TabGroup::modal()` to trap focus in a dialog), give each widget a `TabIndex` (equal indices = child order). Focus state is the `InputFocus` resource; render a focus ring by comparing `focus.get() == Some(entity)` and inserting/removing an `Outline`. Clicking empty space can clear focus:

```rust
.observe(|mut ev: On<Pointer<Click>>, mut focus: ResMut<InputFocus>| {
    focus.clear();
    ev.propagate(false);
})
```
`examples/ui/widgets/tab_navigation.rs`

## Picking: observers

UI nodes are pickable by default (`UiPickingPlugin` is in `DefaultPlugins`). Attach observers per entity; the event's target is `ev.event_target()` (or the `entity` field):

```rust
commands.spawn((Button, Node { .. }))
    .observe(|over: On<Pointer<Over>>, mut q: Query<&mut BackgroundColor>| { .. })
    .observe(|out: On<Pointer<Out>>, ..| { .. })
    .observe(|click: On<Pointer<Click>>, ..| { .. });
```

Event vocabulary: `Over`/`Out` (enter/leave), `Press`/`Release` (button down/up on the entity), `Click` (press+release on same entity), `Move`, `DragStart`/`Drag`/`DragEnd` (on the dragged entity), `DragEnter`/`DragOver`/`DragLeave`/`DragDrop` (on the drop target). Useful payloads: `ev.button` (`PointerButton::Primary/Secondary/Middle`), `drag.delta` (per-frame), `drag.distance` (since DragStart), `drag.pointer_location.position`, `drag_drop.dropped` / `drag_enter.dragged` (the other entity), `ev.hit` (`HitData`: `position`, `normal`, `depth`).

Events bubble up the hierarchy; stop with `ev.propagate(false)` (needs `mut ev: On<...>`). Generic reusable observers: `fn recolor_on<E: EntityEvent>(color: Color) -> impl Fn(On<E>, Query<&mut Sprite>)`.

The older polled API still exists for buttons: query `(&Interaction, ...) , (Changed<Interaction>, With<Button>)` and match `Pressed/Hovered/None`.
`examples/picking/simple_picking.rs`, `examples/ui/widgets/button.rs`, `examples/picking/mesh_picking.rs`

## Pickable behavior and hit-test gotchas

```rust
Pickable::IGNORE                                            // not hoverable, doesn't block lower entities
Pickable { should_block_lower: false, is_hoverable: true }  // hoverable but lets hits pass through
Pickable { should_block_lower: true, is_hoverable: false }  // invisible blocker (click shield)
```

Gotchas that bite:
- **Child nodes re-fire parent hover events.** Text/icons inside a hover target must be `Pickable::IGNORE`, otherwise moving onto the child bubbles a fresh `Over`/`Out` pair on the parent.
- Full-screen root containers should be `Pickable::IGNORE` so they don't swallow world clicks.
- `HoverMap` (per-pointer hovered entity map) **always contains the window entity** — exclude it when checking "is the pointer over UI".
- Sprite picking only hits **opaque pixels** by default (alpha-aware), and in 0.19 sprites need an explicit `Pickable::default()` to be hoverable in the example setup.
- UI picking respects clipping: content scrolled out of an `Overflow::clip/scroll` area is not hit.

`examples/ui/ui_drag_and_drop.rs`, `examples/picking/dragdrop_picking.rs`, `examples/picking/sprite_picking.rs`

## Drag-and-drop UI (inventory-grid style)

Grid of tiles that swap on drop — the whole pattern in observers:

```rust
.observe(|on: On<Pointer<DragStart>>, mut q: Query<(&mut Outline, &mut GlobalZIndex)>| {
    let (mut outline, mut z) = q.get_mut(on.event_target()).unwrap();
    outline.color = Color::WHITE;
    z.0 = 1;                                     // lift above siblings while dragging
})
.observe(|on: On<Pointer<Drag>>, mut q: Query<&mut UiTransform>| {
    q.get_mut(on.event_target()).unwrap().translation = Val2::px(on.distance.x, on.distance.y);
})
.observe(|on: On<Pointer<DragEnd>>, mut q: Query<(&mut UiTransform, &mut GlobalZIndex)>| {
    // snap back; layout position never changed
})
.observe(|on: On<Pointer<DragDrop>>, mut q: Query<&mut Node>| {
    if let Ok([mut a, mut b]) = q.get_many_mut([on.event_target(), on.dropped]) {
        core::mem::swap(&mut a.grid_row, &mut b.grid_row);
        core::mem::swap(&mut a.grid_column, &mut b.grid_column);
    }
});
```

Tiles need `Pickable { should_block_lower: false, is_hoverable: true }` so the dragged tile doesn't block `DragDrop` from reaching the tile underneath. For UI-to-world drops, observe `DragEnter/DragOver/DragLeave/DragDrop` on the world target, check `event.dragged == source_entity`, and use `event.hit.position` to place a ghost preview (`Pickable::IGNORE` on the ghost).
`examples/ui/ui_drag_and_drop.rs`, `examples/picking/dragdrop_picking.rs`

## Mesh and sprite picking backends

- `MeshPickingPlugin` is **not** in `DefaultPlugins` — add it for 3D/2D-mesh picking. By default it raycasts everything; opt-in mode: `MeshPickingSettings { require_markers: true, .. }` then add `Pickable` to camera and targets. Disable window-surface picking with `PickingSettings { is_window_picking_enabled: false, .. }`.
- Backends compose: UI, sprite, and mesh backends coexist; UI on top blocks meshes below (unless `should_block_lower: false`).
- Current hover data per pointer: `Query<&PointerInteraction>` yields `(entity, HitData)` pairs (`hit.position`, `hit.normal`) — good for reticles/decals.
- Custom backends write `PointerHits` in `PreUpdate` within `PickingSystems::Backend`; `HitData` has an `extra` field for arbitrary `Debug + Send + Sync` payloads.

`examples/picking/mesh_picking.rs`, `examples/picking/custom_hit_data.rs`

## Debugging

```rust
// Picking event tracing (bevy_dev_tools feature):
.add_plugins(DebugPickingPlugin)
.insert_resource(DebugPickingMode::Normal)   // cycle Disabled / Normal / Noisy at runtime

// UI layout outlines (bevy_ui_debug feature):
mut debug_options: ResMut<GlobalUiDebugOptions>;  debug_options.toggle();
// Per-node override component:
UiDebugOptions { enabled: true, outline_scrollbars: true, show_clipped: true, line_width: 2., ..default() }
```
`examples/picking/debug_picking.rs`, `examples/testbed/full_ui.rs`, `examples/ui/scroll_and_overflow/scroll.rs`

## Misc

- **Window click-through**: toggle `CursorOptions.hit_test` to let clicks fall through a transparent always-on-top window (`examples/ui/window_fallthrough.rs`).
- **Ghost nodes** (experimental, `ghost_nodes` feature): `GhostNode` entities participate in hierarchy but not layout — useful for logic-only grouping inside UI trees (`examples/ui/layout/ghost_nodes.rs`).
- **Size constraints**: `min_width/max_width/min_height/max_height` on `Node` clamp the flex/grid-resolved size; `aspect_ratio: Some(1.0)` locks proportions (`examples/ui/layout/size_constraints.rs`, `examples/ui/layout/grid.rs`).
