# Weapons & Shields Rendering — Implementation Tasks

> Generated from design doc: `specs/2026-07-01-weapons-rendering/design.md`
> Each task below is **one commit**. Implement top to bottom; respect `Depends on`.

**Goal:** Render right-hand weapon and left-hand shield sprite layers on local and remote players (mirroring headgear), driven by server weapon/shield view ids, with attack SFX from the body ACT.

---

## Progress

- [ ] Task 1: Add `WeaponData` to `lifthrasir-data`
- [ ] Task 2: Add `weapon` converter to `ro-to-lifthrasir-cli` and generate `weapon_data.ron`
- [ ] Task 3: Add `WeaponDb` resource + plugin and register it
- [ ] Task 4: Add weapon/shield sprite-path builders + shield suffix table
- [ ] Task 5: Resolve weapon/shield paths in `handle_equipment_changes`
- [ ] Task 6: Add body-anchored `sync_weapon_layer` system
- [ ] Task 7: Emit weapon/shield events for remote players at spawn
- [ ] Task 8: Emit weapon/shield events for the local player from `Inventory`
- [ ] Task 9: Map `LOOK_WEAPON` sprite changes to weapon + shield events
- [ ] Task 10: Play player body-ACT attack SFX

---

## Task 1: Add `WeaponData` to `lifthrasir-data`

**What:** Introduce the serializable `WeaponData` struct that the CLI writes and the
runtime loads (design §2). Holds `names` (view id → sprite suffix), `hit_sounds`
(view id → hit wav), and `bow_types` (set of bow view ids).

**Code pointers:**
- Modify: `lifthrasir-data/src/lib.rs` — add `pub struct WeaponData { names: BTreeMap<u16,String>, hit_sounds: BTreeMap<u16,String>, bow_types: BTreeSet<u16> }` with `#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]`, mirroring `AccessoryData` (line 36).
- Reference: `lifthrasir-data/src/lib.rs:36` — `AccessoryData` shape and its RON round-trip test at line ~165.

**Acceptance criteria:**
- [ ] `WeaponData` compiles with `BTreeMap`/`BTreeSet` imports present.
- [ ] A unit test serializes a populated `WeaponData` to RON and deserializes it back to an equal value (mirrors the existing `AccessoryData` test).
- [ ] `cargo test -p lifthrasir-data` passes.

**Depends on:** none

**Commit:** `feat(data): add WeaponData struct`

---

## Task 2: Add `weapon` converter to `ro-to-lifthrasir-cli` and generate `weapon_data.ron`

**What:** Add a converter that runs `weapontable.lub` through the mlua VM and extracts
`WeaponNameTable`, `WeaponHitWaveNameTable`, and `BowTypeList` into `weapon_data.ron`
(design §2). Commit the regenerated RON.

**Code pointers:**
- Create: `ro-to-lifthrasir-cli/src/converters/weapon.rs` — `pub fn run(vfs, out)`: `exec_chunk` on `data/luafiles514/lua files/datainfo/weapontable.lub` (decompiling via the same `read_grf_lub` bytecode-magic check), read the three globals, `decode_euckr` the string values, write `weapon_data.ron`.
- Modify: `ro-to-lifthrasir-cli/src/converters/mod.rs:20-25` — add `pub mod weapon;` and `("weapon", weapon::run)` to the `CONVERTERS` array.
- Reference: `ro-to-lifthrasir-cli/src/converters/accessory.rs` — `read_grf_lub`, `decode_euckr`, `lua::exec_chunk`, RON write pattern (whole file).
- Create: `assets/data/ron/weapon_data.ron` — generated output (commit it), same dir as `accessory_data.ron`.

**Acceptance criteria:**
- [ ] `cargo run -p ro-to-lifthrasir-cli -- convert --only weapon` (or equivalent invocation) writes `weapon_data.ron` and prints a non-zero entry count.
- [ ] The generated RON deserializes into `WeaponData` with `names[2] == "_검"` and a non-empty `hit_sounds` and `bow_types`.
- [ ] A converter unit/integration test asserts the extracted `names` contains a known class suffix and `hit_sounds` contains `_hit_sword.wav`.

**Depends on:** Task 1

**Commit:** `feat(cli): add weapon table converter`

---

## Task 3: Add `WeaponDb` resource + plugin and register it

**What:** Load `weapon_data.ron` at runtime into a `WeaponDb` resource exposing
`suffix(view_id)`, `hit_sound(view_id)`, and `is_bow(view_id)` (design §2), mirroring
`AccessoryDb`.

**Code pointers:**
- Create: `game-engine/src/infrastructure/weapon/asset.rs` — `WeaponDataAsset(pub lifthrasir_data::WeaponData)` (mirror `accessory/asset.rs`).
- Create: `game-engine/src/infrastructure/weapon/registry.rs` — `WeaponDb` resource with `from_weapon_data`, `suffix`, `hit_sound`, `is_bow` + unit tests.
- Create: `game-engine/src/infrastructure/weapon/plugin.rs` — `WeaponDbPlugin` loading `data/ron/weapon_data.ron` (mirror `accessory/plugin.rs`).
- Create: `game-engine/src/infrastructure/weapon/mod.rs` — re-export the three, mirror `accessory/mod.rs`.
- Modify: `game-engine/src/infrastructure/mod.rs` — add `pub mod weapon;`.
- Modify: `game-engine/src/lib.rs:23` and `:52` — `pub use infrastructure::weapon::{WeaponDb, WeaponDbPlugin};` and `.add(WeaponDbPlugin)`.

**Acceptance criteria:**
- [ ] `WeaponDb::suffix` returns the correct suffix for a known view id and `None` for an unknown one (unit test, mirrors `accessory/registry.rs` tests).
- [ ] `WeaponDataAsset` deserializes an inline RON fixture (unit test).
- [ ] `cargo build -p game-engine` succeeds and `WeaponDbPlugin` is added once in `lib.rs`.

**Depends on:** Task 1, Task 2

**Commit:** `feat(engine): add WeaponDb resource and plugin`

---

## Task 4: Add weapon/shield sprite-path builders + shield suffix table

**What:** Add pure path builders for weapon and shield SPR/ACT, plus the hardcoded
shield view→suffix table with a numeric fallback (design §2, §3). Builders take job
sprite name + gender + suffix and format the `ro://` path.

**Code pointers:**
- Modify: `game-engine/src/domain/assets/patterns.rs` — add `weapon_sprite_path(gender, job_name, suffix)` → `ro://data/sprite/인간족/{job}/{job}_{sex}{suffix}.spr`, `weapon_action_path(...)` (`.act`), `shield_sprite_path(gender, job_name, suffix)` → `ro://data/sprite/방패/{job}/{job}_{sex}_{suffix}_방패.spr`, `shield_action_path(...)`, and `shield_suffix(view_id) -> String` (1→가드, 2→쉴드, 3→버클러, 4→미러쉴드, else the id as a string).
- Reference: `game-engine/src/domain/assets/patterns.rs:28` (`body_sprite_path`) and `:100` (`headgear_sprite_path`) — gender→`남`/`여` mapping and `format!` style.

**Acceptance criteria:**
- [ ] Unit test: `weapon_sprite_path(Male, "검사", "_검")` == `ro://data/sprite/인간족/검사/검사_남_검.spr` and `"_1116"` yields `..._남_1116.spr`.
- [ ] Unit test: `shield_sprite_path(Male, "검사", &shield_suffix(1))` == `ro://data/sprite/방패/검사/검사_남_가드_방패.spr` and `shield_suffix(28901)` == `"28901"`.
- [ ] `.act` variants produce the same paths with the `.act` extension.

**Depends on:** none

**Commit:** `feat(sprite): add weapon/shield sprite path builders`

---

## Task 5: Resolve weapon/shield paths in `handle_equipment_changes`

**What:** Branch `handle_equipment_changes` per slot so weapon/shield view ids resolve
to SPR/ACT via `WeaponDb`/`shield_suffix` + the job sprite name, requesting the layer
animation the same way headgear does (design §3). Headgear path unchanged.

**Code pointers:**
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/events.rs:24` — keep `resolve_headgear_paths`; add `resolve_weapon_paths(weapon_db, job_name, gender, view_id)` and `resolve_shield_paths(job_name, gender, view_id)` returning `Option<(String,String)>`.
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/events.rs:50-127` — add `&CharacterData`, `Res<JobSpriteRegistry>`, `Option<Res<WeaponDb>>` to `handle_equipment_changes`; select the resolver by `event.slot` (Weapon/Shield vs the three head slots). Use `JobSpriteRegistry::get_sprite_name(job_id)` for the job folder name.
- Reference: `game-engine/src/domain/entities/sprite_rendering/systems/spawn.rs:96-120` — how `JobSpriteRegistry` + `job_id` produce the body path (job name source).
- Reference: `game-engine/src/infrastructure/job/registry.rs` — `get_sprite_name` / `get_body_sprite_path`.
- Reference: `game-engine/src/domain/entities/character/components/core.rs:11` — `CharacterData` component carrying `job_id`.

**Acceptance criteria:**
- [ ] Unit tests for `resolve_weapon_paths` (known view id → expected weapon SPR/ACT, unknown → `None`) and `resolve_shield_paths` (classic + renewal suffix).
- [ ] A weapon/shield `EquipmentChangeEvent` with a known view id requests a `pending_animations` entry tagged `LAYER_WEAPON`/`LAYER_SHIELD`; a `None` view id despawns the existing slot layer (existing behavior preserved).
- [ ] Headgear resolution and its existing tests still pass; `cargo test -p game-engine` passes.

**Depends on:** Task 3, Task 4

**Commit:** `feat(sprite): resolve weapon and shield sprite paths`

---

## Task 6: Add body-anchored `sync_weapon_layer` system

**What:** Position weapon/shield layers each frame against the body's per-frame anchor
(`BodyAttachPoint`), not the head's, so they ride the body like the head does (design
§4). Registered after `sync_player_body_layer`.

**Code pointers:**
- Create: `game-engine/src/domain/entities/sprite_rendering/systems/weapon_sync.rs` — `sync_weapon_layer` handling `EquipmentSlot::Weapon` and `EquipmentSlot::Shield`, with `#[auto_add_system(plugin = SpriteRenderingDomainPlugin, schedule = Update, config(in_set = SpriteRenderingSystems::TransformUpdate, after = sync_player_body_layer))]`.
- Reference: `game-engine/src/domain/entities/sprite_rendering/systems/head_sync.rs:81` (`sync_player_head_layer`) — the body-anchor math (`BodyAttachPoint`, `head_screen_offset`, `head_billboard_delta`) to copy.
- Reference: `game-engine/src/domain/entities/sprite_rendering/systems/headgear_sync.rs` — camera filter, z-offset push, `RenderLayer.equipment_slot` gating (but anchor to head — do NOT copy that part).
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/mod.rs` — add `pub mod weapon_sync;` and re-export `sync_weapon_layer`.

**Acceptance criteria:**
- [ ] System gates on `render_layer.equipment_slot` ∈ {Weapon, Shield} and reads `BodyAttachPoint` (not `HeadAttachPoint`).
- [ ] Registered exactly once and ordered after `sync_player_body_layer` (compiles under the auto-plugin macro).
- [ ] Manual: with a weapon layer present (via Task 5 + an emitter), the weapon tracks the body across the 8 directions and attack frames, rendering over the body.

**Depends on:** Task 5

**Commit:** `feat(sprite): sync weapon and shield layers to body anchor`

---

## Task 7: Emit weapon/shield events for remote players at spawn

**What:** Drive remote players' equipped weapon/shield through the renderer by emitting
`EquipmentChangeEvent`s from their `EquipmentSet` on spawn (design §5), mirroring
headgear.

**Code pointers:**
- Modify: `game-engine/src/domain/entities/spawning/remote_headgear.rs` — add `weapon_shield_view_ids(equipment) -> Vec<(EquipmentSlot,u16)>` for `EquipmentSlot::Weapon`/`Shield` (mirror `headgear_view_ids` at line 9) and emit those events in `emit_remote_headgear_events` (line 35).
- Reference: `game-engine/src/domain/entities/character/components/equipment.rs` — `EquipmentSet.weapon` / `.shield` fields.

**Acceptance criteria:**
- [ ] `weapon_shield_view_ids` returns the weapon/shield slots for non-zero view ids and skips zero/absent (unit test, mirrors the headgear test in the same file).
- [ ] On remote spawn with a weapon/shield, matching `EquipmentChangeEvent`s are written.
- [ ] Manual: a remote player wearing a weapon/shield renders both.

**Depends on:** Task 5, Task 6

**Commit:** `feat(sprite): render remote players' weapons and shields`

---

## Task 8: Emit weapon/shield events for the local player from `Inventory`

**What:** Extend the local-player inventory reconciliation to emit weapon/shield changes
using the right/left-hand equip bitmasks (design §5), so login/equip/unequip drive the
weapon and shield layers.

**Code pointers:**
- Modify: `game-engine/src/domain/equipment/local_headgear.rs` — extend `LocalHeadgearApplied` (line ~15) with `weapon`/`shield` fields, `desired_headgear` (line ~86) or a sibling to also read `EQP_RIGHT_HAND`/`EQP_LEFT_HAND`, and add `reconcile` calls for the two slots in `sync_local_player_headgear`.
- Reference: `game-engine/src/domain/equipment/location.rs:4,8` — `EQP_RIGHT_HAND` / `EQP_LEFT_HAND` masks.

**Acceptance criteria:**
- [ ] The desired-set helper returns the weapon/shield view ids for the right/left-hand equipped items (unit test with a fixture inventory).
- [ ] Equipping then unequipping a weapon emits an add event then a remove (`view_id: None`) event; no event when unchanged (reconcile guard).
- [ ] Manual: the local player's weapon/shield appear/disappear on equip/unequip.

**Depends on:** Task 5, Task 6

**Commit:** `feat(sprite): render local player's weapon and shield`

---

## Task 9: Map `LOOK_WEAPON` sprite changes to weapon + shield events

**What:** Handle the `LOOK_WEAPON = 2` sprite-change look type for remote players, which
uniquely carries both weapon (`val`) and shield (`val2`) views, emitting one event per
slot including zero (unequip) so the layer despawns (design §5).

**Code pointers:**
- Modify: `game-engine/src/domain/equipment/sprite_change.rs:9-20` — add `const LOOK_WEAPON: u32 = 2;`; since it maps to two slots, handle it explicitly in `apply_sprite_changes` (line ~34) rather than through the single-slot `headgear_slot` helper.
- Modify: `game-engine/src/domain/equipment/sprite_change.rs:34+` — on `LOOK_WEAPON`, emit a Weapon event from `change.val` and a Shield event from `change.val2` (each `0 → view_id: None`); keep the local-player skip.
- Reference: `net-contract/src/events/zone.rs` — `UnitSpriteChanged { val, val2 }`.

**Acceptance criteria:**
- [ ] Unit test: a `UnitSpriteChanged` with `type_ = 2`, `val = W`, `val2 = S` maps to a Weapon event (`Some(W)`) and a Shield event (`Some(S)`).
- [ ] Unit test: `val2 = 0` still emits a Shield event with `view_id: None` (unequip despawns the layer).
- [ ] Existing headgear look-type tests still pass.

**Depends on:** Task 5, Task 6

**Commit:** `feat(sprite): map weapon look-type sprite changes`

---

## Task 10: Play player body-ACT attack SFX

**What:** Give players a `SpatialAudioEmitter` and let `sync_player_body_layer` emit its
per-frame `PlayMobSfx`, so the body ACT's attack sound plays for players (design §6).
Weapon ACTs carry no sound, so no weapon-layer SFX is added.

**Code pointers:**
- Modify: `game-engine/src/domain/entities/sprite_rendering/systems/body_sync.rs:106-145` — pass `Some(&mut sfx_writer)` in `sync_player_body_layer` instead of `None` (the frame-crossing SFX path already exists in `sync_body_layer_impl`).
- Modify: `game-engine/src/domain/entities/spawning/systems.rs` — insert `SpatialAudioEmitter::default()` on spawned player character roots (the entity `PlayMobSfx` targets), mirroring the mob insert at line ~234.
- Reference: `game-engine/src/domain/audio/systems.rs:290` (`play_mob_sfx`) — emitter consumption; `game-engine/src/domain/character/systems.rs` — `SpatialAudioReceiver` on the local player (already present).

**Acceptance criteria:**
- [ ] Player character roots carry a `SpatialAudioEmitter` after spawn.
- [ ] `sync_player_body_layer` writes `PlayMobSfx` on attack frames that carry a `sound_id`.
- [ ] Manual: attacking as/near a player plays the body attack sound; `cargo test` and `cargo clippy` pass.

**Depends on:** none

**Commit:** `feat(audio): play player attack SFX from body ACT`

---

## Out of scope (design §7)

- Two-handed weapon shield suppression — handled server-side (`shield_view = 0`).
- Bow ranged-attack body animation — `WeaponDb.is_bow` is captured but unused.
- Riding/mounted job sprites, weapon effect/glow overlays — excluded.
- Per-weapon-type hit-wave SFX (`WeaponHitWaveNameTable`) — table captured in Task 2/3, wired-but-unused; fast follow-up.
- Equipment-window preview weapon/shield cache — small follow-up.
