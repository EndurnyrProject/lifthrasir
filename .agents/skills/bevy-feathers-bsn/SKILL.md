---
name: bevy-feathers-bsn
description: "Use when building Bevy 0.19 UI with BSN scenes (the bsn! macro, scene functions, Children[], on() observers, #Name child-entity capture, SceneComponents) and/or bevy_feathers widgets (FeathersPlugins, UiTheme token overrides, @FeathersButton/@FeathersCheckbox, ThemeBackgroundColor/ThemeTextColor). Trigger whenever authoring or reviewing a Bevy 0.19 window/panel/widget, porting imperative bevy_ui spawning to bsn!, wiring a Feathers theme, or debugging why a bsn! tree won't compile, a Feathers button does nothing, or themed text/colors don't apply. Covers the idiomatic patterns, the real 0.19 API gaps, and the bundle-factory anti-pattern that makes BSN look worse than imperative."
---

# Bevy 0.19 BSN scenes + Feathers

The next-generation scene system (BSN) and the `bevy_feathers` widget toolkit, as they actually ship in **Bevy 0.19** (verified against the released crates and the official `examples/scene/bsn.rs` + `examples/large_scenes/bevy_city/` examples). This is the modern, declarative way to author `bevy_ui` — use it for new windows/panels/widgets instead of imperative `commands.spawn` trees.

> Lifthrasir context: the equipment window (`lifthrasir-ui/src/widgets/equipment_window/`) is the canonical worked example in this repo — read `scene.rs` for a full declarative window, `mod.rs` for plugin/toggle/markers, `slots.rs` for the data-sync-via-system pattern, and `theme/feathers_theme.rs` for the token overrides. See [[lifthrasir-bevy-plugins]] for the rest of the ecosystem (those are still Bevy 0.18-era notes; this skill is 0.19).

## The one mistake to avoid

`bsn!` is **not** a fancy bundle macro. If you write `commands.spawn_scene(node()).insert(ChildOf(x)).id()` once per entity and parent every edge by hand, you have re-implemented imperative spawning with extra ceremony — it comes out *longer* and not declarative. That is the trap. The whole value of BSN is **composing scene functions with `Children [...]` and attaching behavior with `on(...)`** so the hierarchy reads top-to-bottom in one expression. Author the whole window as one `bsn!` tree, then parent it with a **single** `.insert(ChildOf(parent))`.

## Setup

BSN ships in `bevy_scene` and is re-exported in `bevy::prelude` — **no extra dependency**. The `2d`/`3d`/`ui` default features pull the `scene` feature, and `ScenePlugin` is already in `DefaultPlugins`. You get `bsn!`, `bsn_list!`, `Scene`, `SceneList`, `template_value`, `on`, and `CommandsSceneExt::spawn_scene`.

Feathers needs an **explicit dependency** and is **not** re-exported by `bevy`:

```toml
bevy_feathers = "0.19.0"   # use via the bevy_feathers::... path
```

```rust
use bevy_feathers::{FeathersPlugins, FeathersCorePlugin, theme::{UiTheme, ThemeToken},
                    dark_theme::create_dark_theme};

// The plugin is the FeathersPlugins GROUP + FeathersCorePlugin — NOT a `FeathersPlugin`.
// Install your custom UiTheme BEFORE adding the group: FeathersCorePlugin does
// init_resource::<UiTheme>(), which will NOT overwrite a theme you already inserted.
app.insert_resource(my_theme());           // your overrides first
if !app.is_plugin_added::<FeathersCorePlugin>() {
    app.add_plugins(FeathersPlugins);
}
```

### Theme tokens

`UiTheme(ThemeProps)`; the palette is `theme.0.color: HashMap<ThemeToken, Color>`. Start from `create_dark_theme()` (it populates every control token) and override what you need. `ThemeToken::new_static("name")` is `const`, so you can declare your own named tokens. `theme.color(token)` looks one up.

```rust
pub const TOKEN_WINDOW_BG: ThemeToken = ThemeToken::new_static("myapp/window_bg");

pub fn my_theme() -> UiTheme {
    let mut theme = UiTheme(create_dark_theme());
    theme.0.color.insert(TOKEN_WINDOW_BG, MY_GLASS);        // your palette
    theme.0.color.insert(bevy_feathers::tokens::WINDOW_BG, MY_GLASS); // override a built-in
    theme
}
```

Keep your own palette constants as the source-of-truth values feeding the token map, so non-Feathers code and the tokens stay in sync.

## Idiomatic BSN, by example

The shape to copy (condensed from `examples/scene/bsn.rs` and bevy_city `settings.rs`):

```rust
fn window() -> impl Scene {
    bsn! {
        MyWindowRoot                              // marker: a bare unit-struct literal
        Node { width: px(540), flex_direction: FlexDirection::Column }
        ThemeBackgroundColor(TOKEN_WINDOW_BG)     // theme token component (see Theming)
        Visibility::Hidden                        // enum literal (Visibility derives VariantDefaults)
        Children [ titlebar(), body(), footer() ] // compose scene fns — THIS is the point
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        MyTitlebar Node { /* row */ }
        on(on_titlebar_drag)                      // attach an observer fn
        Children [
            label("Equipment"),
            ( @FeathersButton { @caption: bsn! { glyph("close") } }
              on(|_: On<Activate>, mut q: Query<&mut Visibility, With<MyWindowRoot>>| {
                  if let Ok(mut v) = q.single_mut() { *v = Visibility::Hidden; }
              }) ),
        ]
    }
}
```

Spawn the whole tree and parent it with ONE insert:

```rust
commands.spawn_scene(window()).insert(ChildOf(hud_root));
```

## Flex-first layout

Prefer normal flex flow for window chrome, stacked sections, tab bodies, forms, and scroll content. Use absolute positioning only for true overlays or anchored controls that flex cannot express.

- Declare `flex_direction` on every structural container. Use `Column` for window/body stacks and `Row` for controls that belong on one line.
- Make column containers stretch their children with `align_items: AlignItems::Stretch`. Give full-width children `width: percent(100)` when the inherited size is otherwise ambiguous; use `flex_grow: 1.0` and `min_width: px(0)` for flexible row children.
- Bound scroll regions with a flex size or explicit height, then set `overflow`. Do not build the primary panel layout from absolute insets.
- **`Visibility::Hidden` does not remove an entity from layout.** For mutually exclusive tabs, pages, or modes, set the inactive `Node.display` to `Display::None` and the active one to `Display::Flex`. Optionally mirror `Visibility` as well when existing systems/tests depend on it.
- Never leave several `height: percent(100)` siblings in one flex container and hide the inactive ones only with `Visibility`; Bevy still flex-shrinks all of them, producing tiny panels and stray scrollbars.
- Keep `EditableText` fields in normal flex flow, stretched to the form width. Do not attach `Pickable::IGNORE` to an input; pointer focus is required for refocusing and native Backspace/Delete editing.
- Check the exact picking settings type before diagnosing marker behavior: `MeshPickingSettings::require_markers` affects 3D mesh picking, not Bevy UI. Only `UiPickingSettings::require_markers` controls UI marker requirements.
- **Every `EditableText` needs a `TabIndex`** (on itself or an ancestor). Feathers installs `TabNavigationPlugin`, whose global `click_to_focus` observer fires an `AcquireFocus` on every pointer press; the event bubbles up looking for a `TabIndex`, and if it reaches the window it **clears `InputFocus`** — silently undoing the click-to-focus that `EditableTextInputPlugin`'s own press observer just performed, in the same frame. Feathers' own text-input control ships with `TabIndex`; a bare `EditableText` does not. In Lifthrasir this is handled once via `register_required_components::<EditableText, TabIndex>()` in `UiFocusMirrorPlugin`. Symptom of the missing index: Enter/programmatic focus works, clicking the field does nothing.
- Do not pass a `ResMut<T>` as `&mut T` into a helper that runs every frame and usually does nothing: the deref-mut coercion alone flags the resource as changed each frame, breaking every `is_changed` consumer (Feathers gates its focus-indicator scan on `InputFocus::is_changed`). Pass `&mut ResMut<T>` and only deref-mut on the write path.

For tabbed UI, leave exactly one page participating in layout:

```rust
let active = selected == Tab::Members;
*visibility = if active { Visibility::Inherited } else { Visibility::Hidden };
node.display = if active { Display::Flex } else { Display::None };
```

Add one structural test that asserts exactly one page has `Display::Flex` and inactive pages have `Display::None`. For editable fields, assert the field carries `TabIndex` — that is the piece whose absence breaks click-to-focus.

### Hierarchy — `Children [ ... ]`

- `Children [ a(), b() ]` — comma-separated entries are sibling entities.
- Listing components **without** a comma adds them to the *same* entity: `Children [ Marker Node {..} Text("x") ]` is one child with three components.
- Wrap a multi-component entry in parens for clarity: `Children [ (Marker Node {..}), other() ]`.
- Nest arbitrarily deep; entries can themselves be `fn() -> impl Scene` calls. This composition is the core idiom — build small reusable scene functions (`button(label)`, `slot_well(spec)`) and assemble them.
- **Dynamic lists**: build a `Vec<impl Scene>` and embed it as `Children [ {my_vec} ]`.

### Observers — `on(...)`

Attach an entity observer inline; the first param's type selects the event:

```rust
on(|e: On<Pointer<Press>>, mut settings: ResMut<Settings>| { /* ... */ })
on(my_handler_fn)   // or a named fn
```

Multiple `on(...)` on one entity each add a separate observer. This replaces imperative `.observe()`. Note `Pointer<Drag>`/`Pointer<Press>` **bubble** up the hierarchy — an observer on a parent fires for descendant targets, so guard with `if titlebars.get(drag.entity).is_err() { return }` when you only want the event from a specific entity (e.g. a drag handle, not its child buttons).

### Feathers widgets — the `@` SceneComponent syntax

Feathers buttons/checkboxes are **SceneComponents**, spawned with `@`:

```rust
@FeathersButton { @caption: bsn! { Text("Regenerate") ThemedText } }
on(|_: On<Activate>, ...| { /* ... */ })

@FeathersCheckbox { @caption: bsn! { Text("Simulate Cars") ThemedText } }
on(checkbox_self_update)                                   // built-in visual toggle
on(|c: On<ValueChange<bool>>, mut s: ResMut<Settings>| { s.cars = c.value; })
```

- A Feathers `@FeathersButton` emits `bevy_ui_widgets::Activate` on press — observe `On<Activate>`, **not** `On<Pointer<Click>>`. A mismatched event type = a dead button.
- `@FeathersCheckbox` emits `On<ValueChange<bool>>`; pair it with `on(checkbox_self_update)` for the built-in toggle visual.
- `@caption` takes a nested `bsn!` scene (text or an icon node).

### Theming components

Apply tokens directly with these components (from `bevy_feathers`):

- `ThemeBackgroundColor(token)`, `ThemeBorderColor(token)`, `ThemeTextColor(token)` — set a node's bg/border/text color from a theme token. Prefer these over raw `BackgroundColor(color)` for anything that should follow the theme.
- `ThemedText` — makes text **inherit** font + color from a Feathers ancestor (`InheritableFont`/`InheritableThemeTextColor`). It only works under such an ancestor (e.g. inside a Feathers widget). Standalone window text has no inheritable ancestor, so use `ThemeTextColor(token)` for color directly.

### Asset paths as string literals

`Handle<T>` fields accept a string literal when the component derives `FromTemplate`; it becomes a `HandleTemplate` that calls `AssetServer::load` at resolve time (de-duped against already-loaded handles). No need to thread `AssetServer` through scene functions:

```rust
ImageNode { image: "ui/icons/sword.png" }
TextFont { font: FontSourceTemplate::Handle("fonts/Title.ttf"), font_size: px(15.0) }
```

### Markers and enums

- Unit-struct markers (`#[derive(Component, Default, Clone)]`) are **bare literals** in `bsn!`: just write `MyMarker`.
- For a component to be `bsn!`-usable it must derive `Default` + `Clone`, or `FromTemplate`.
- Enums need a default **per variant**. `bsn!` looks for `default_{variant_lower}()` static methods; deriving `Default` with a `#[default]` variant covers it. To pass a *runtime* enum value (not a literal), use `template_value(my_enum)` — that requires the enum derive `Default` so its `Template` impl exists.

### `template_value(x)` — when you actually need it

`template_value` is the escape hatch for values that aren't simple patchable literals: a runtime variable, or a complex constructor like `Transform::from_xyz(..).looking_at(..)`. You do **not** need it for `BackgroundColor(c)`, `BorderColor::all(c)`, or enum literals like `Visibility::Hidden`. Reaching for `template_value` on everything is a symptom of the bundle-factory anti-pattern. (It is a plain fn call — wrapping its argument in extra `{}` trips the `unnecessary_braces` lint; only `Node` *field values* take `{}`.)

### Capturing child entity ids — `#Name` + `FromTemplate`

A common need: a parent component must hold the `Entity` ids of specific children (so a system can patch them later). Do it declaratively — name the children in the scene scope and reference them:

```rust
#[derive(Component, FromTemplate)]
struct SlotParts { glyph: Entity, icon: Entity, refine: Entity, name: Entity }

fn slot_well() -> impl Scene {
    bsn! {
        SlotKind                                  // (a marker / runtime value)
        SlotParts { glyph: #Glyph, icon: #Icon, refine: #Refine, name: #SlotName }
        Node { /* well */ }
        Children [
            (#Glyph  ImageNode { image: "ui/icons/glyph.png" }),
            (#Icon   ImageNode { /* hidden until filled */ }),
            (#Refine Text("")),
            (#SlotName Text("Empty")),
        ]
    }
}
```

`#Name` in a `bsn!` scope creates an entity reference resolvable within that same scope (and into descendants). A missing `#Name` is a spawn-time error, not a silent null. This beats any post-spawn capture pass.

## Reactivity: dynamic data stays a system

**BSN 0.19 has no automatic data binding.** A `SceneComponent` composes *structure* (it spawns an associated sub-scene), it does not re-render when some resource changes. The docs even note caching isn't wired for function-scenes/SceneComponents.

The idiomatic way to update dynamic content is exactly what Bevy's own `bevy_city` does for its loading text: author the node with a marker in `bsn!`, then run a normal change-detection system that queries by marker and mutates:

```rust
// bsn!:  LoadingText Text("Loading...")
fn update_loading(mut q: Query<&mut Text, With<LoadingText>>, /* data */) {
    let Ok(mut text) = q.single_mut() else { return };
    text.0 = format!(/* ... */);
}
```

So: **structure + behavior in `bsn!`; live data in a system keyed on marker components.** Don't try to force reactive data through SceneComponents — you'll be fighting 0.19.

## Real 0.19 API gaps (don't hunt for nonexistent tokens)

- **No font theme token.** Fonts in Feathers are inheritance-only (`ThemedText` via an ancestor). Standalone text must set its font explicitly (`FontSourceTemplate::Handle(...)`).
- **No `ImageNode` tint token** (no `ThemeImageColor`). Icon/image colors stay raw palette values; they don't follow the theme.
- **`.bsn` asset format is not shipped.** Scenes are authored in Rust via the `bsn!` macro (and `#[scene("file.bsn")]` on a SceneComponent is forward-looking only).
- **Feathers is experimental.** Widget set and theming APIs may shift across versions; verify against the pinned crate source when something doesn't match.

## Quick checklist when authoring a window

1. One flex-first `bsn!` tree of composable `fn() -> impl Scene`; `Children [...]` for hierarchy; one `.insert(ChildOf(parent))` to mount it. Reserve absolute positioning for overlays.
2. Interactions via `on(...)`; Feathers buttons via `@FeathersButton { @caption: ... }` observing `On<Activate>`.
3. Colors via `ThemeBackgroundColor/ThemeBorderColor/ThemeTextColor`; fonts/icons via asset-path string literals.
4. Child-id capture via `#Name` + a `FromTemplate` parent component.
5. Live data via a marker-querying system, not SceneComponents.
6. Inactive flex pages use `Display::None`; `Visibility::Hidden` alone is not a layout toggle.
7. Every `EditableText` carries a `TabIndex` (else Feathers' `click_to_focus` clears `InputFocus` right after the click sets it). Do not confuse UI marker settings with mesh marker settings.
8. `template_value` only for runtime values / complex constructors.

## Reference

- This repo's worked example: `lifthrasir-ui/src/widgets/equipment_window/{scene,mod,slots}.rs` + `theme/feathers_theme.rs`.
- Official: `examples/scene/bsn.rs`, `examples/large_scenes/bevy_city/src/{main,settings}.rs` (tag `v0.19.0`). Fetch with
  `gh api "repos/bevyengine/bevy/contents/examples/scene/bsn.rs?ref=v0.19.0" --jq '.content' | base64 -d`.
- Crate docs: `bevy_scene` lib.rs (the module docs cover Children, `#Name`, `on()`, asset paths, SceneComponent) and `bevy_feathers` `controls/button.rs` + `theme.rs`.
