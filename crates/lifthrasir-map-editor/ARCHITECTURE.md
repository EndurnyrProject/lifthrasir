# Editor architecture

The editor is a single package with an inward-facing document API and an
optional Bevy executable. Keep editor policy here, not in the game client or
shared runtime. No client or network plugin is required.

## Modules and responsibilities

```text
src/lib.rs                     Public document API (no ECS types)
src/document/
  mod.rs                       Collision/prop editing, dirty state, undo/redo
  persistence.rs               LIF validation, GLB encoding, atomic disk writes
src/main.rs                    CLI and dependency/plugin composition only
src/application.rs             Session resources, history/save commands, ordering
src/infrastructure/content.rs  ro:// source and shared world-renderer adapter
src/presentation/
  mod.rs                       Presentation plugin composition
  viewport.rs                  Camera, map activation/lighting, infinite grid
  collision.rs                 Cell input, cursor, collision visualization
  props/
    mod.rs                     Duplicate/delete and document-to-scene sync
    keyboard.rs                Keyboard transform input
    selection.rs               Picking and selection visualization
    gizmo/mod.rs               Mouse gestures, mode and undo transaction boundary
    gizmo/view.rs              Gizmo visuals and pick volumes
  ui.rs                        BSN help/notification scenes
```

`MapDocument` owns document mutations and history. Presentation translates
Bevy inputs into that API and synchronizes the resulting state to entities.
History commands never query the viewport; prop and collision modules own
their own visual synchronization. Only the explicit Ctrl+S command calls
`MapDocument::save`. Ending a drag commits history, **not** a file write.

Update sets run in this order: `Edit -> History -> Save -> Present`. Pointer
observers remain Bevy's immediate input path. Each feature plugin owns its
startup entities, resources, observers and systems; the entry point does not
reach into individual features.

The persistence implementation is separate from editing logic while preserving
the existing `MapDocument::open`/`save` public API. It retains atomic replacement
and stable in-memory node identities. There is no speculative repository trait
or second document model: introduce a storage port if a second backend is
actually needed.

## Dependencies and standalone extraction

The document library requires only `lifthrasir-data` (the Bevy-free LIF schema)
and general-purpose serialization/error/tempfile crates. It can be checked and
tested without the GUI:

```sh
cargo check --locked -p lifthrasir-map-editor --no-default-features --lib
cargo test --locked -p lifthrasir-map-editor --no-default-features --lib
```

The default `gui` feature adds Bevy, CLI/bootstrap configuration and the shared
asset/runtime crates. Workspace asset-source and renderer composition, and the
runtime components the viewport consumes, are gathered in
`infrastructure/content.rs`. This is an integration seam, not a claim of
complete renderer independence: its component re-exports intentionally expose
the existing shared scene contract to presentation.

Keeping the loader and world renderer avoids copying glTF/LIF extension
handling, prop loading, lighting/effect and asset-source implementations.
Extracting the editor later means carrying these shared crates or replacing
that content adapter and its scene contract. Editing/history and the rest of
the editor need no game-client dependency. This refactor does not modify other
workspace crates or move pak build/pruning policy into the editor.

## Validation

```sh
cargo check --locked -p lifthrasir-map-editor --all-targets
cargo test --locked -p lifthrasir-map-editor
cargo test --locked -p lifthrasir-map-editor --no-default-features --lib
```

Headless regression tests cover document history/storage, explicit-only saving,
gizmo release and right-button filtering, keyboard transforms, camera/grid
settings and query initialization. They do not validate rendered appearance,
retail assets or native mouse/cursor behavior; those require an interactive
editor session.
