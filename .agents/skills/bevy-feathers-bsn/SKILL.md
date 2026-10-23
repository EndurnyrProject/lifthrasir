---
name: bevy-feathers-bsn
description: "Use when building Bevy 0.20 UI with BSN scenes (the bsn!/bsn_list! macros, @scene includes, -- separated list entities, on() observers, #Name child-entity capture, SceneComponent props, On<Ready>) and/or bevy_feathers widgets (FeathersPlugins, UiTheme + semantic tokens + ThemeContext, @FeathersButton/@FeathersCheckbox/@FeathersSelect/@FeathersNumberInput, headless TabList/Tab, ThemeBackgroundColor/ThemeTextColor). Trigger whenever authoring or reviewing a Bevy 0.20 window/panel/widget, porting imperative bevy_ui spawning to bsn!, wiring a Feathers theme or contextual theming, or debugging why a bsn! tree won't compile, a Feathers button does nothing, click-to-focus doesn't work, or themed text/colors don't apply. Covers the idiomatic patterns, the real 0.20 API gaps, and the bundle-factory anti-pattern that makes BSN look worse than imperative."
---

# Bevy 0.20 BSN scenes + Feathers

The next-generation scene system (BSN) and the `bevy_feathers` widget toolkit as they ship in **Bevy 0.20**. This is the modern, declarative way to author `bevy_ui` — use it for new windows/panels/widgets instead of imperative `commands.spawn` trees.

> Lifthrasir context: the windows under `crates/lifthrasir-client-ui/src/widgets/` are the worked examples (`guild_window/`, `shop_window/`, `storage_window/`, `trade_window/` are the largest), and `crates/lifthrasir-ui/src/theme/feathers_theme.rs` is the theme source of truth. For non-UI ecosystem crates see [[lifthrasir-bevy-plugins]]; for core ECS/rendering see [[bevy]].

## 0.20 syntax migration (this repo already migrated — don't reintroduce the old forms)

If you copy a snippet from older notes or from a 0.19 codebase, it will not compile. Four changes:

| 0.19 (dead) | 0.20 |
|---|---|
| `scene_function()` in a scene body | `@scene_function()` |
| `scene_variable` / `{scene_expression}` as a scene include | `@scene_variable` / `@{scene_expression}` |
| `template_value(x)` | just `x`; if that fails, `~x` (`template_value` is deprecated) |
| `Children [ a(), (b c), d() ]` | `Children [ @a() -- b c -- @d() ]` |

Also: `bsn_list! []` / `bsn_list! ()` are discouraged — use `bsn_list! {}` (the only form `rustfmt` leaves alone). `#[derive(VariantDefaults)]` no longer exists.

Symptom → cause:

- `expected ``@``` / "expected expression" on a scene-name identifier → missing `@`.
- "expected `--`" or "expected `,`"-era errors inside `Children [ .. ]` → entities must be separated by `--`; commas and grouping parens are deprecated.
- Enum variant missing fields → 0.20 removed per-variant defaults; specify every field.

## The one mistake to avoid

`bsn!` is **not** a fancy bundle macro. If you write `commands.spawn_scene(node()).insert(ChildOf(x)).id()` once per entity and parent every edge by hand, you have re-implemented imperative spawning with extra ceremony — it comes out *longer* and not declarative. That is the trap. The whole value of BSN is **composing scene functions with `Children [...]` and attaching behavior with `on(...)`** so the hierarchy reads top-to-bottom in one expression. Author the whole window as one `bsn!` tree, then parent it with a **single** `.insert(ChildOf(parent))`.

## Setup

BSN ships in `bevy_scene`, is re-exported in `bevy::prelude`, and `ScenePlugin` is already in `DefaultPlugins`. You get `bsn!`, `bsn_list!`, `Scene`, `SceneList`, `on`, `Ready`, and `CommandsSceneExt::spawn_scene` / `queue_spawn_scene`.

Feathers is available two ways — pick one and stay consistent:

- **This repo's way:** the standalone crate (`bevy_feathers = "0.20.0"` in the workspace), imported as `bevy_feathers::…`.
- Since 0.19/0.20 `bevy` also re-exports it behind its `bevy_feathers` feature, so `use bevy::feathers::…` works without a separate dependency. (An older version of this skill claimed it was *not* re-exported — that was wrong.)

```rust
use bevy_feathers::{FeathersPlugins, FeathersCorePlugin, theme::UiTheme};

// FeathersPlugins = TabNavigationPlugin + FeathersCorePlugin.
// FeathersCorePlugin does init_resource::<UiTheme>(), so insert your theme FIRST.
app.insert_resource(my_theme());
if !app.is_plugin_added::<FeathersCorePlugin>() {
    app.add_plugins(FeathersPlugins);
}
```

**0.20 must-do in this repo: add `PointerFocusPlugin` yourself.** Feathers' `TabNavigationPlugin` supplies tab navigation, but 0.20 moved pointer/click focus into `bevy_input_focus::pointer_focus::PointerFocusPlugin` and it is **not** part of `FeathersPlugins` or `InputFocusPlugin`. Without it a click clears focus at the window and never acquires it:

```rust
app.add_plugins((
    InputFocusPlugin,
    InputDispatchPlugin,
    PointerFocusPlugin,   // 0.20: click-to-focus lives here now
));
```

## Theming — design tokens → semantic tokens → color

0.20 split the theme into two lookup stages. `create_dark_theme()` now returns **`ThemeProps`** (not `UiTheme`), and its shape is:

```rust
ThemeProps {
    token_assignments: HashMap<ThemeToken, SemanticToken>,          // design token -> semantic token
    semantic_base: HashMap<SemanticToken, Color>,                   // semantic token -> color
    semantic_overrides: HashMap<SurfaceLevel, HashMap<SemanticToken, Color>>,  // per-context colors
}
```

So this 0.19 code no longer compiles: `theme.0.color.insert(token, color)` — there is no `color` map. The repo helper is the pattern to copy:

```rust
use bevy_feathers::theme::{SemanticToken, ThemeProps, ThemeToken, UiTheme};
use bevy_feathers::dark_theme::create_dark_theme;

/// Point `design` at its own semantic token with `color`. One semantic token per
/// design token keeps an override from leaking into every control that shares a
/// Feathers semantic token.
pub fn assign_token(theme: &mut ThemeProps, design: ThemeToken, color: Color) {
    let semantic = SemanticToken::new(design.to_string().into());
    theme.token_assignments.insert(design, semantic.clone());
    theme.semantic_base.insert(semantic, color);
}

pub fn norse_theme() -> UiTheme {
    let mut theme = create_dark_theme();              // populates every built-in token
    assign_token(&mut theme, TOKEN_WINDOW_BG, GLASS);
    assign_token(&mut theme, bevy_feathers::tokens::WINDOW_BG, GLASS);  // override a built-in
    UiTheme(theme)
}
```

Lookups: `UiTheme::color(&token)` (base context, backwards-compatible) and `UiTheme::context_color(&token, SurfaceLevel)`. An unknown token logs `warn_once!` and renders **fuchsia** — a bright magenta UI element means a token is not assigned.

### Contextual theming

`ThemeContext(SurfaceLevel)` on a container propagates to descendants, so the same token resolves to a different color inside a dialog vs. a pane. `SurfaceLevel` is `Base` (window/pane header), `Higher` (pane body), `Highest` (groups/subpane header), `Floating` (dialog/menu). Fill `semantic_overrides` per level when you want that, and set `ThemeContext` on the container:

```rust
bsn! {
    Node { /* dialog */ }
    ThemeContext(SurfaceLevel::Floating)
    Children [ /* widgets inside inherit the Floating palette */ ]
}
```

## Idiomatic BSN, by example

```rust
fn window() -> impl Scene {
    bsn! {
        MyWindowRoot                              // marker: bare unit-struct literal
        Node { width: px(540), flex_direction: FlexDirection::Column }
        ThemeBackgroundColor(TOKEN_WINDOW_BG)     // theme token component
        Visibility::Hidden                        // enum literal (Default + Clone is enough)
        Children [
            @titlebar()                           // @ = include a scene function
            --
            @body()
            --
            @footer()
        ]
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        MyTitlebar Node { /* row */ }
        on(on_titlebar_drag)                      // attach an observer fn
        Children [
            @label("Equipment")
            --
            @FeathersButton {
                @caption: bsn! { @icon("ui/icons/x.png") }
            }
            on(|_: On<Activate>, mut q: Query<&mut Visibility, With<MyWindowRoot>>| {
                if let Ok(mut v) = q.single_mut() { *v = Visibility::Hidden; }
            })
        ]
    }
}
```

Spawn the whole tree and parent it with ONE insert:

```rust
commands.spawn_scene(window()).insert(ChildOf(hud_root));
// or, to wait for asset dependencies:
commands.queue_spawn_scene(window());
```

### Hierarchy — `Children [ ... ]`

- `--` **separates entities**. Whitespace only **adds components to the same entity**: `Children [ Marker Node {..} Text("x") ]` is one child with three components.
- `Children [ @a() -- Marker Node {..} -- @b() ]` is three children. No parens, no commas.
- Nest arbitrarily deep; entries can be `fn() -> impl Scene` calls. Build small reusable scene functions (`button(label)`, `slot_well(spec)`) and assemble them — that composition is the point.
- **Dynamic lists**: build a `Vec<impl Scene>` (or a `SceneList`) and include it as `@{my_vec}`.
- Any `RelationshipTarget` works the same way: `MyRel [ a -- b ]`, not just `Children`.

### Observers — `on(...)`

Attach an entity observer inline; the first param's type selects the event:

```rust
on(|e: On<Pointer<Press>>, mut settings: ResMut<Settings>| { /* ... */ })
on(my_handler_fn)   // or a named fn
```

Multiple `on(...)` on one entity each add a separate observer; this replaces imperative `.observe()`. `Pointer<Drag>`/`Pointer<Press>` **bubble**, so an observer on a parent fires for descendant targets — guard with `if titlebars.get(drag.entity).is_err() { return }` when you only want the event from a specific entity (e.g. a drag handle, not its child buttons).

### `On<Ready>` — run logic once the scene is fully spawned

0.20 adds `Ready`, triggered per entity after that entity **and all its descendants** are spawned (including loaded scene dependencies). This is the bottom-up counterpart to `On<Add>`, which fires top-down before children exist. Use it instead of guessing when children are available:

```rust
bsn! {
    Node { width: px(100), height: px(100) }
    on(|ready: On<Ready>| { /* full hierarchy is present here */ })
    Children [ Text("hello") ]
}
```

### Feathers widgets — `@` + props

Feathers controls are `SceneComponent`s included with `@`. Props are prefixed `@`, real component fields are not:

```rust
@FeathersButton {
    @caption: bsn! { @label("Regenerate") },
    @variant: ButtonVariant::Primary,
}
on(|_: On<Activate>, ...| { /* ... */ })

@FeathersCheckbox { @caption: bsn! { @label("Simulate Cars") } }
on(checkbox_self_update)                                   // built-in visual toggle
on(|c: On<ValueChange<bool>>, mut s: ResMut<Settings>| { s.cars = c.value; })
```

`SceneComponent`s are defined as: `#[derive(SceneComponent, Default, Clone)] #[scene(MyProps)]` plus `fn scene(props: MyProps) -> impl Scene`. Props fields typed `Box<dyn SceneList>` take `bsn_list! { … }` (or a single `bsn! { … }`).

Widgets available in 0.20 (`bevy_feathers::controls`, `::containers`, `::display`):

| Widget | Notes / events |
|---|---|
| `FeathersButton`, `FeathersToolButton` | `On<Activate>`; props `@caption`, `@variant`, `@corners` |
| `FeathersCheckbox`, `FeathersRadio`, `FeathersToggleSwitch` | `On<ValueChange<bool>>` + `checkbox_self_update` / `radio_self_update` |
| `FeathersSlider`, `FeathersColorSlider` | `On<ValueChange<f32>>` + `slider_self_update` |
| `FeathersNumberInput` | `On<ValueChange<…>>`; scrub/drag **and** typing; see limits below |
| `FeathersTextInput` | wraps `EditableText` + `TextInput`; props `@visible_width`, `@max_characters` |
| `FeathersSelect` | new dropdown; `@options` (list of `FeathersListRow`), `@max_visible` |
| `FeathersColorInput`, `FeathersColorSwatchGrid`, `FeathersColorWheel` | new color editing (RGB/HSL + hex + recent swatches) |
| `FeathersListView`, `FeathersScrollbar` | scrollable/selectable list; scrollbar props `@target` (`#Name`), `@orientation`, `@auto_hide` |
| `FeathersMenu`, `FeathersLazyMenu`, `FeathersMenuButton`, `FeathersMenuToolButton`, `FeathersMenuItem` | lazy menu spawns/despawns the popup instead of hiding a pre-spawned one |
| `FeathersDialog`, `FeathersFloatingDialog` (+ header/body/footer/close) | `RequestClose` |
| `FeathersDisclosureToggle` | expand/collapse |

Behavior notes that bite:

- A Feathers button emits `bevy_ui_widgets::Activate` — observe `On<Activate>`, **not** `On<Pointer<Click>>`. A mismatched event type = a dead button.
- All Feathers widgets are **controlled**: they emit `ValueChange<T>` and expect your handler to write the resource back. `*_self_update` observers only handle the widget's own visuals.
- `FeathersNumberInput` limits are separate optional components: `HardLimit` (clamps typing and dragging), `SoftLimit` (dragging only; adds the slider look), `NumberInputPrecision` (drag quantization), `Step`.

### Text input — `EditableText` + `TextInput` (split in 0.20)

`EditableText` (`bevy::text`) is now **state only**; the widget behavior moved to `TextInput` (`bevy::ui_widgets`). A working field needs both, plus the plugin:

```rust
use bevy::text::{EditableText, EditableTextFilter};
use bevy::ui_widgets::TextInput;

bsn! {
    EditableText::new("1")
    TextInput
    EditableTextFilter::new(|c| c.is_ascii_digit())
    TabIndex(0)
    Node { height: px(32) }
}
// app: app.add_plugins(bevy::ui_widgets::TextInputPlugin);
```

**Every text field needs a `TabIndex`** (on itself or an ancestor), in addition to `PointerFocusPlugin`. `click_to_focus` triggers a bubbling `AcquireFocus`; it focuses the target only if it carries a `TabIndex`, and otherwise the request reaches the window and *clears* focus. Feathers' `FeathersTextInput` ships its own `TabIndex`; a bare `EditableText` does not. In this repo it is handled once:

```rust
app.register_required_components::<EditableText, TabIndex>();
```

Symptom of the missing index: Enter/programmatic focus works, clicking the field does nothing.

Keep `EditableText` fields in normal flex flow. Do **not** put `Pickable::IGNORE` on an input — pointer focus needs the press.

### Headless tabs (0.20) vs. manual page toggling

`bevy_ui_widgets` now has visuals-free tabs. Selection is owned by the app: `SelectedTab` lives on the `TabList`, interaction emits `ValueChange<Option<Entity>>`, and the optional `tablist_self_update` observer applies it. Keyboard: arrow keys (roving tab index), Home/End, Enter/Space. `TabActivation::Automatic` selects on focus move; the default `Manual` separates focus from selection.

```rust
TabList { orientation: ControlOrientation::Horizontal, activation: TabActivation::Automatic }
SelectedTab(Some(first_tab))
on(tablist_self_update)
Children [
    Tab Children [ Text("General") ]
    --
    Tab Children [ Text("Rendering") ]
]
```

For a `#Name` target the example wraps it, because `SelectedTab` takes an `OptionTemplate`:

```rust
fn selected_tab(tab: EntityTemplate) -> impl Scene {
    let tab = OptionTemplate::Some(tab);
    bsn! { SelectedTab({tab}) }
}
// … Children [ @selected_tab(#general) -- @tab_header("Rendering") ]
```

For hand-rolled windows (what this repo does), leave exactly one page in layout and toggle `display` — `Visibility` alone is not a layout toggle:

```rust
let active = selected == Tab::Members;
*visibility = if active { Visibility::Inherited } else { Visibility::Hidden };
node.display = if active { Display::Flex } else { Display::None };
```

Add one structural test asserting exactly one page has `Display::Flex`. Never leave several `height: percent(100)` siblings all participating in layout.

### Theming components

- `ThemeBackgroundColor(token)`, `ThemeBorderColor(token)`, `ThemeTextColor(token)` — set bg/border/text from a theme token. Prefer these over raw `BackgroundColor(color)`.
- `InheritableThemeTextColor(token)` — for a themed ancestor that descendants (`ThemedText`) inherit from.
- `ThemedText` — text **inherits** font + color from a Feathers ancestor. It only works under such an ancestor (e.g. inside a Feathers widget); standalone window text has no inheritable ancestor, so use `ThemeTextColor` directly.

### Sizing units and the font-size default

0.20 added `Val::Em`/`Val::Rem` with `em(…)`/`rem(…)` helpers. `Val::Em` resolves against the node's `EmSize` (derived from its `TextFont`; `Node` auto-requires `EmSize`), `Val::Rem` against the global `RemSize` resource.

**`TextFont::default()` is now `FontSize::Rem(1.)`, not `FontSize::Px(20.)`.** With the default `RemSize` of 20px that renders identically, but any text left at the default size now scales with `RemSize`. To keep a fixed size, say so:

```rust
TextFont { font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"), font_size: {FontSize::Px(12.0)} }
```

`font_size` is the `FontSize` enum (`Px`/`Vw`/`Vh`/`VMin`/`VMax`/`Rem`), never a bare `f32`.

### Border radius

`BorderRadius` fields are `CornerRadius` (an `x`/`y` `Val` pair) so corners can be elliptical:

```rust
BorderRadius::all(px(8))                                   // circular, unchanged API
BorderRadius::top_right(CornerRadius::new(px(10.), px(20.))) // elliptical
BorderRadius::MAX / ::MAX_ELLIPTICAL / ::ZERO              // still consts
```

### `FixedNode` — viewport-anchored UI

`FixedNode` (with `Node`) positions relative to the camera viewport instead of the parent, and inherits no layout/clipping/transform context — it behaves like a root node. Use it for HUD overlays that must not move with a scrolling panel. Rotated/scaled ancestors now clip correctly (CPU Sutherland–Hodgman clipping) — the overflow API is unchanged.

### Asset paths as string literals

`Handle<T>` fields accept a string literal when the component derives `FromTemplate`; it becomes a handle template resolved by `AssetServer::load` at resolve time (de-duped). No need to thread `AssetServer` through scene functions:

```rust
ImageNode { image: "ui/icons/sword.png" }
```

### Markers and enums

- Unit-struct markers (`#[derive(Component, Default, Clone)]`) are **bare literals**: just write `MyMarker`.
- Anything used as a `bsn!` value needs `Default` + `Clone`, or a `FromTemplate` derive.
- Enums only need `#[derive(Component, Default, Clone)]` with a `#[default]` variant. **Every field must be spelled out** — 0.20 removed the per-variant `default_{variant}()` machinery:

```rust
Foo::A { x: 1, y: 0 }   // not Foo::A { x: 1 }
```

### Values and the `~` escape hatch

Component values are literals, variables, function calls, and chained method calls — `component_variable`, `component_fn()`, `Transform::from_xyz(..).looking_at(..)`, `node.clone()` are all fine with **no wrapper**. `{}` is only needed for an inline expression (`Node { width: {some_var + 3.} }`).

`template_value` is deprecated in 0.20; prefer deleting it. If a value genuinely needs the `Template`/`InsertTemplate` path, prefix with `~`: `~MyType { name: 1 }` or `~{ my_expr }`.

### Capturing child entity ids — `#Name` + `FromTemplate`

A parent component holding child `Entity` ids is still the right pattern — name the children in the scene scope and reference them:

```rust
#[derive(Component, FromTemplate)]
struct SlotParts { glyph: Entity, icon: Entity, refine: Entity, name: Entity }

fn slot_well() -> impl Scene {
    bsn! {
        SlotKind
        SlotParts { glyph: #Glyph, icon: #Icon, refine: #Refine, name: #SlotName }
        Node { /* well */ }
        Children [
            #Glyph ImageNode { image: "ui/icons/glyph.png" }
            --
            #Icon ImageNode { /* hidden until filled */ }
            --
            #Refine Text("")
            --
            #SlotName Text("Empty")
        ]
    }
}
```

`#Name` is scoped to one `bsn!` invocation (and visible to descendants); a missing `#Name` is a spawn-time error, not a silent null. Components should store the resolved `Entity`, never an `EntityTemplate` — derive `FromTemplate` to convert.

## Flex-first layout

Prefer flex flow for window chrome, stacked sections, tab bodies, forms, and scroll content; use absolute positioning only for true overlays.

- Declare `flex_direction` on every structural container (`Column` for stacks, `Row` for one-line controls).
- Stretch columns with `align_items: AlignItems::Stretch`; use `flex_grow: 1.0` + `min_width: px(0)` for flexible row children, and `width: percent(100)` when the inherited size is ambiguous.
- Bound scroll regions with a flex size or explicit height, then set `overflow`.
- Do not pass a `ResMut<T>` as `&mut T` into a per-frame helper that usually does nothing: the deref-mut coercion alone marks the resource changed, breaking every `is_changed` consumer (Feathers gates its focus-indicator scan on `InputFocus::is_changed`). Pass `&mut ResMut<T>` and only deref-mut on the write path.
- `MeshPickingSettings::require_markers` affects 3D mesh picking, **not** UI. Only `UiPickingSettings::require_markers` controls UI marker requirements.

## Reactivity: dynamic data stays a system

BSN has no automatic data binding. A `SceneComponent` composes *structure*; it does not re-render when a resource changes. Author the node with a marker in `bsn!`, then mutate it from a change-detection system:

```rust
// bsn!:  LoadingText Text("Loading...")
fn update_loading(mut q: Query<&mut Text, With<LoadingText>>) {
    let Ok(mut text) = q.single_mut() else { return };
    text.0 = "Loaded".to_string();
}
```

So: **structure + behavior in `bsn!`; live data in a system keyed on marker components.** Don't try to force reactive data through `SceneComponent` props.

## Real 0.20 API gaps (don't hunt for nonexistent tokens)

- **No font theme token.** Fonts are inheritance-only (`ThemedText` under an ancestor). Standalone text must set its font explicitly via `FontSourceTemplate::Handle(...)`.
- **No `ImageNode` tint token** (no `ThemeImageColor`). Icon colors stay palette values; they don't follow the theme.
- **`.bsn` asset files are not shipped**, and cached scene includes (`:"file.bsn"`, `:scene()`) are still unimplemented — both are compile-time or spawn-time warnings in the syntax docs. Author scenes in Rust.
- **Feathers is experimental.** The widget set and theme API shift between minors; verify against `crates/bevy_feathers` rather than memory.

## Quick checklist when authoring a window

1. One flex-first `bsn!` tree of composable `fn() -> impl Scene`; `Children [ … -- … ]` for hierarchy; one `.insert(ChildOf(parent))` (or `spawn_scene`) to mount it.
2. Every scene include uses `@` (`@fn()`, `@var`, `@{expr}`, `@SceneComp`); list entities are separated by `--`; no `template_value`.
3. Interactions via `on(...)`; Feathers buttons via `@FeathersButton { @caption: … }` observing `On<Activate>`.
4. Colors via `ThemeBackgroundColor`/`ThemeBorderColor`/`ThemeTextColor`; `assign_token` when adding a token; `ThemeContext` for dialog surfaces.
5. Child-id capture via `#Name` + a `FromTemplate` parent component.
6. Live data via a marker-querying system, not `SceneComponent` props.
7. Inactive flex pages use `Display::None`; `Visibility::Hidden` alone is not a layout toggle.
8. Editable fields carry `EditableText` **and** `TextInput`, plus `TabIndex`; `PointerFocusPlugin` + `TextInputPlugin` are installed.
9. Fixed sizes are explicit (`FontSize::Px`), because the default is now `rem(1)`.

## Reference

- Repo: `crates/lifthrasir-client-ui/src/widgets/{guild_window,shop_window,storage_window,trade_window}/scene.rs` (large declarative windows), `crates/lifthrasir-ui/src/theme/feathers_theme.rs` (`assign_token`, `norse_theme`), `crates/lifthrasir-ui/src/plugin.rs` (`UiPrimitivesPlugin`: `PointerFocusPlugin`, `register_required_components::<EditableText, TabIndex>`).
- Official examples at tag `v0.20.0`: `examples/scene/bsn.rs`, `examples/ui/widgets/{feathers_gallery,feathers_number_input,headless_tabs,tab_navigation}.rs`, `examples/large_scenes/bevy_city/src/settings.rs`.
- Crate docs: `bevy_scene_macros` (`bsn!` / `bsn_list!` syntax tables — the authoritative reference), `bevy_scene` (concepts, `Ready`, `SceneComponent`), `bevy_feathers` (`theme` for `ThemeProps`/`ThemeContext`, `controls` for per-widget props). On docs.rs these are `bevy::scene`, `bevy::feathers`; in a vendored/cargo-registry checkout they are `crates/bevy_scene/src/lib.rs` and `crates/bevy_feathers/src/{lib,theme}.rs`.
