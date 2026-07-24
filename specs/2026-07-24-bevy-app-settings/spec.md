# Bevy App Settings Migration

## Summary

Replace Lifthrasir's custom settings persistence with Bevy 0.19's built-in App
Settings framework. Graphics, audio, and keybinds will become independent
Bevy-managed settings resources while the existing settings window and its
Apply/Cancel/Reset behavior remain intact.

## Context & Problem

Lifthrasir currently owns settings persistence through `bevy-persistent`, a
custom platform-specific path, startup loading and recovery logic, and a
`Persistent<Settings>` resource wrapper. That wrapper is exposed throughout the
engine and UI, so consumers that only need graphics, audio, or keybinds depend
on the aggregate persistence mechanism as well.

Bevy 0.19 now provides an official
[App Settings framework](https://bevy.org/news/bevy-0-19/#app-settings). It loads
plain settings groups as ECS resources and supplies commands for saving them.
Keeping Lifthrasir's parallel persistence layer would duplicate upstream
functionality, retain unnecessary dependencies, and make future Bevy
integration harder.

If nothing changes, Lifthrasir continues to own storage location, serialization,
loading, recovery, and wrapper types that Bevy now handles directly.

## Goals & Non-Goals

### Goals

- Use Bevy 0.19 App Settings as the sole settings persistence framework.
- Expose graphics, audio, and keybinds as three independent settings resources.
- Remove `bevy-persistent`, settings-domain use of `dirs`, the custom settings
  persistence module, and the persisted aggregate `Settings` wrapper.
- Preserve all existing settings, defaults, and controls.
- Preserve the settings window's draft, dirty-state, Apply, Cancel, and Reset
  behavior.
- Preserve live application of committed settings to graphics, audio, input,
  assets, and newly spawned game entities.
- Leave storage location, format, atomic writes, and platform behavior to Bevy.

### Non-Goals

- Migrating, reading, rewriting, or deleting the old `settings.ron`.
- Maintaining source or runtime compatibility with `Persistent<Settings>` or
  the aggregate persisted `Settings` resource.
- Redesigning the settings window.
- Adding, removing, or changing the meaning of individual settings.
- Removing domain-specific glue that applies settings to Bevy windows,
  rendering, audio, assets, or input.
- Redesigning the separate per-character hotbar persistence mechanism.
- Building an additional abstraction over Bevy App Settings.

## Considered Options

### 1. Independent native groups with the existing Apply/Cancel workflow

Graphics, audio, and keybinds become separate Bevy settings resources. The UI
continues to edit an isolated draft and commits the three groups only when the
user selects Apply.

This removes the custom persistence layer and broad aggregate resource while
preserving established user behavior. It was selected because consumers
already use settings by domain and the draft remains useful for avoiding
expensive live graphics changes while browsing options.

### 2. Replace persistence but retain one aggregate settings resource

The existing nested `Settings` value could become a single Bevy settings group,
minimizing the initial code change.

This was rejected because it would retain a broad custom container whose
consumers depend on unrelated settings. It would adopt Bevy's storage mechanism
without adopting its useful resource model.

### 3. Mutate and save settings immediately

Controls could edit native settings resources directly and use Bevy's deferred
save command, eliminating most draft and Apply/Cancel logic.

This was rejected because it changes established settings-window behavior and
could repeatedly reconfigure rendering or rebuild assets during editing.

## Chosen Direction

Lifthrasir will have three independently persisted settings groups:

- Graphics settings
- Audio settings
- Keybind settings

Bevy will load them at application startup and expose them as ordinary ECS
resources. Consumers will read only the group they need rather than a
persistence wrapper or aggregate resource.

The settings window will retain non-persisted draft copies of all three groups.
Its behavior will be:

- **Apply:** copy changed draft values into the active resources, request a Bevy
  save, apply the committed values to the running game, and clear the dirty
  state.
- **Cancel:** restore the draft from the active resources and cancel any
  in-progress key capture.
- **Reset:** replace the draft with built-in defaults without affecting the
  running game until Apply.
- **Close without Apply:** leave active and persisted settings unchanged.

Freshly created cameras, lights, terrain, effects, character visuals, and player
input will continue to initialize from the active domain settings.

Apply acknowledges an in-memory commit and a queued persistence request. It
does not wait for disk I/O confirmation. Bevy owns atomic writing and error
reporting.

The old RON file is outside the migration: it remains untouched and is no
longer read by Lifthrasir.

## Success Criteria

- Production and test code no longer reference `bevy-persistent`,
  `Persistent<Settings>`, or the custom settings path/loading module.
- The workspace no longer depends on `bevy-persistent`; settings code no longer
  uses `dirs`. The existing hotbar persistence remains its only intended
  `dirs` consumer.
- Graphics, audio, and keybinds load as independent Bevy-managed resources.
- Existing default values and user-visible controls remain unchanged.
- Applying changes updates the running game and requests persistence for all
  changed groups.
- Applied values load correctly on a subsequent launch.
- Cancel, Reset, dirty-state indication, audio controls, graphics controls, and
  input rebinding continue to behave as before.
- Runtime consumers no longer depend on an aggregate persisted settings type.
- Settings required during later entity or asset creation remain available to
  those flows.
- Lifthrasir contains no migration or compatibility path for the old RON
  schema.

## Constraints

- Target Bevy 0.19's built-in App Settings API.
- Use `com.github.endurnyrproject.lifthrasir` as the stable application
  identifier, derived from the repository origin.
- Settings must be loaded before plugins and startup behavior that require
  their values.
- Persistence must use Bevy's native save commands and platform storage.
- Per-character hotbar persistence remains unchanged and may continue using
  `dirs`.
- The Apply/Cancel/Reset interaction model is binding.
- Backward compatibility with the previous resource types, file location, and
  RON schema is explicitly out of scope.
- App-specific runtime synchronization remains permitted only where Bevy cannot
  apply a setting automatically.

## Critique Findings

The critique reconsidered whether retaining Apply/Cancel needlessly preserved
custom machinery. Immediate mutation would delete more UI state, but it would
also change deliberate user behavior and repeatedly apply potentially expensive
graphics settings. Retaining a small, non-persisted draft is therefore accepted;
it is UI state, not a second persistence layer.

The old implementation could keep the UI dirty when a synchronous persistence
write failed. Bevy's normal save flow is asynchronous, so preserving that exact
guarantee would require recreating custom persistence coordination. The
accepted behavior clears the dirty state once settings are committed in memory
and saving is requested; Bevy logs persistence failures.

Deleting the obsolete RON file was also considered. It is deliberately left
untouched because deleting user data is unnecessary for the migration and
would add legacy-file handling to a design whose explicit goal is to remove it.

## Open Questions

None. Storage format and location are intentionally delegated to Bevy rather
than specified by Lifthrasir.
