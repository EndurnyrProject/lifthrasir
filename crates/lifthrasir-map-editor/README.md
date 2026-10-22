# Lifthrasir Map Editor

GLB-native map editor for Lifthrasir. The editor works on converted
Lifthrasir map files and does not load or write Ragnarok `RSW`, `GND`, `GAT`, or
GRF files.

## Run

From the workspace root:

```bash
cargo run -p lifthrasir-map-editor
```

Select another converted map with a path relative to `assets/`:

```bash
cargo run -p lifthrasir-map-editor -- \
  --map data/maps/prontera/prontera.glb
```

The editor expects the map to contain the current `LIF_map` format and the
editor-owned `LIF_editor` collision payload. Reconvert stale maps before
opening them:

```bash
cargo run -p ro-to-lifthrasir-cli -- convert-map --map prontera --force-models
```

## Current controls

### Editor UI

The editor uses Dear ImGui (the game client continues to use Bevy/Feathers).
The File menu opens and saves converted maps. Clicking **Settings** opens a
draggable, resizable window with a live terrain-grid thickness slider and color
picker. Grid appearance changes are session-only; `P` toggles the grid.
Mouse/keyboard input captured by editor windows is excluded from viewport
editing and camera controls.

Presentation code is organized by responsibility:

- `src/presentation/ui/`: Dear ImGui composition, windows, file actions, and input capture.
- `src/presentation/viewport.rs`: camera setup and map lighting/framing.
- `src/presentation/terrain_grid.rs`: terrain-only grid geometry and appearance.
- `src/presentation/props/` and `collision.rs`: viewport editing tools.

Dear ImGui's local `imgui.ini` window layout is ignored by Git. Headless UI tests
disable layout persistence so they do not overwrite the user's window positions.

### Camera

- `W/A/S/D`: fly relative to the camera orientation
- Right mouse drag: look around
- Mouse wheel: change camera height
- `Ctrl` blocks camera movement while using editor shortcuts

### Collision

- Left click terrain: select a collision cell
- Arrow keys: move the selected collision cell
- `Space`: toggle walkability
- `Ctrl+Z`: undo
- `Ctrl+Y`: redo
- `Ctrl+S`: save

The collision overlay is currently experimental and may be disabled/parked
while prop editing is developed.

### Props

- Click a prop: select it
- `I/K`: move along Z
- `J/L`: move along X
- `U/O`: move along Y
- `Q/E`: rotate around Y
- `Delete`: delete the selected prop
- `Ctrl+Z` / `Ctrl+Y`: undo/redo prop edits

Prop movement is coalesced into one history entry per key-held gesture. After
the transform keys are released, the editor waits 150 ms and automatically
saves the map. `Ctrl+S` also commits and saves immediately.

## Editing model

`MapDocument` in `src/document.rs` is the editor document model. It:

1. Reads the GLB header, JSON chunk, and BIN chunk directly.
2. Decodes `LIF_map`, `LIF_editor`, and optional `LIF_water` metadata.
3. Keeps decoded collision cells in memory.
4. Tracks collision edits, prop transforms, and prop deletions in one undo/redo
   history.
5. Saves through a temporary file in the map directory, then atomically
   replaces the original GLB.

The Bevy application in `src/main.rs` is the visual editor shell. It loads the
map through the normal Lifthrasir runtime so converted props, emitters,
lighting, and map metadata are displayed the same way as in the client.

## File format

### `LIF_editor`

`LIF_editor` currently owns the editable collision grid. Each cell contains:

- four little-endian `f32` corner heights;
- one byte of gameplay flags: walkable, water, and snipable.

The collision bytes are stored in a glTF buffer view. They are not GAT bytes
and contain no Ragnarok file header. Runtime loading converts this payload to a
temporary compatibility altitude representation for existing pathfinding and
grounding APIs.

### Prop deletion

Deleted props are tracked in editor memory and omitted from the saved GLB node
table. Scene-root and child node indices are remapped while saving. The
separate prop model GLB is not automatically deleted yet; asset pruning is a
separate planned operation.

## Important implementation details

- The runtime exposes `LifMapNodeIndex` on map entities. It preserves the
  original GLB node identity so editor transforms can be written back to the
  correct JSON node.
- Prop transforms are edited as glTF local `translation`, `rotation`, and
  `scale` values.
- Map props are leaf nodes in the current converted map representation, which
  makes node-table compaction safe for the current editor delete operation.
- The editor uses the workspace `assets/` directory as Bevy's asset root even
  though Cargo runs the package from `crates/lifthrasir-map-editor/`.
- The runtime asset source is registered as `ro://` using `assets/loader.toml`.
- Save and error messages appear in the lower-left notification area and fade
  after three seconds.

## Known limitations

- Prop controls are keyboard-based; there is no mouse transform gizmo yet.
- Prop deletion is not yet exposed as a full scene/document reload workflow.
- Collision editing is cell-based and does not yet sculpt visual terrain.
- Terrain material/texture painting is not implemented.
- Adding, duplicating, or replacing props is not implemented.
- Deleted prop model GLBs are not automatically removed from the asset folder or
  pak.
- The editor currently assumes a valid converted GLB and current `LIF` format;
  migration tools for older editor documents do not exist yet.

## Roadmap

### 1. Editing reliability

- Add structural tests for deleted-node GLB compaction and scene-index remapping.
- Add a reload/reopen command and unsaved-change confirmation.
- Make save errors preserve the in-memory document and clearly identify the
  affected path.
- Ensure every edit type participates in the same coalesced history model.

### 2. Prop authoring

- Add a mouse transform gizmo with translate and rotate modes.
- Add axis constraints, snapping, numeric transform fields, and reset actions.
- Add prop metadata panel: node name, model path, transform, animation values.
- Add duplicate prop and replace-model operations.
- Add a searchable converted-model browser.

### 3. Terrain and collision

- Render a reliable collision grid overlay using the map's actual terrain
  coordinate conversion.
- Add brush selection and multi-cell collision editing.
- Add terrain height sculpting and regenerate terrain mesh data.
- Add terrain texture/material painting and preserve editor-owned terrain data
  in a new `LIF_editor` schema section.

### 4. World objects

- Edit lights, effects, and audio emitters.
- Add/delete emitters and edit their metadata.
- Add visual previews for effect and audio assets.
- Add map-level environment and lighting controls.

### 5. Asset management

- Scan map GLBs for referenced prop models.
- Provide a dry-run unused-model report.
- Add an explicit prune command with confirmation and backup support.
- Integrate edited maps into pak build validation.

### 6. Editor workflow

- Add multi-map project/open dialogs.
- Add editor camera bookmarks and focus-selected commands.
- Add selection outlines and a hierarchy/scene panel.
- Add import/export validation reports.
- Add snapshot/backup history for map documents.

## Validation

Useful checks while developing:

```bash
cargo fmt --check
cargo test --locked -p lifthrasir-map-editor
cargo check --locked -p lifthrasir-map-editor
cargo check --locked -p lifthrasir-runtime
```

The editor should be tested against a freshly converted map after any change
to `lif::FORMAT_VERSION` or the GLB schema.
