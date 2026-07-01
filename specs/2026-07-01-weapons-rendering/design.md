# Weapons & Shields Rendering — Design

Date: 2026-07-01
Status: Approved

## 1. Overview & scope

Render the right-hand **weapon** and left-hand **shield** as animated sprite
layers on both **local and remote players**, mirroring the existing headgear
pipeline. Weapon attack **SFX** reuses the per-frame ACT `sound_id` mechanism the
body/mob path already uses.

Much of the plumbing already exists and is unused for these two slots:

- `EquipmentSlot::Weapon` / `EquipmentSlot::Shield`
  (`game-engine/src/domain/entities/character/components/equipment.rs`)
- z-order tags `LAYER_WEAPON = 40`, `LAYER_SHIELD = 50`
  (`game-engine/src/domain/sprite/tags.rs`) and `equipment_slot_to_tag` /
  `tag_to_equipment_slot` mapping both directions.
- `finalize_equipment_layers`
  (`game-engine/src/domain/entities/sprite_rendering/systems/events.rs`) already
  spawns a `RenderLayer` billboard child for **any** equipment tag once its
  SPR+ACT load.
- `EquipmentSet.weapon` / `.shield` are already populated from `UnitEntered`
  (`game-engine/src/domain/entities/spawning/systems.rs`).

What is missing: weapon/shield path resolution, a body-anchored sync system, event
emitters for the two new slots, and player SFX.

### Server contract (verified against aesir)

- The server sends a **weapon *view* id** (weapon-class / weapon-appearance id),
  not an item id — resolved server-side via `view_of(nameid)` from the item DB.
  Same model as headgear view ids.
- Weapon and shield collapse into a single **look slot** `LookType.weapon = 2`:
  on a live change, `val` carries the weapon view and `val2` carries the shield
  view in one `UnitSpriteChanged`
  (`net-contract/.../events/zone.rs` `UnitSpriteChanged`).
- Two-handed weapons force `shield_view = 0` server-side, so the client never
  needs to suppress a shield for a two-hander.
- A `0` view id means bare-handed / no shield → render no layer (not an error).

## 2. Data pipeline — `WeaponDb` from `weapontable.lub`

`weapontable.lub` (`data/luafiles514/lua files/datainfo/weapontable.lub`) is
compiled Lua bytecode. It defines:

- **`Weapon_IDs`** — the `WEAPONTYPE_*` / `WPCLASS_*` enum of view ids.
- **`WeaponNameTable`** — `view_id -> sprite suffix`, e.g.
  `1 -> _단검` (dagger), `2 -> _검` (sword), `3 -> _양손검` (two-hand sword),
  `4 -> _창` (spear) … and collection weapons `-> _1116`, `_1207`, etc. Suffixes
  already carry a leading `_`.
- **`WeaponHitWaveNameTable`** — `weapon type -> hit wav`, e.g. `_hit_sword.wav`,
  `_hit_spear.wav`, `_hit_axe.wav`, `_hit_mace.wav`, `_hit_rod.wav`,
  `_hit_arrow.wav`.
- **`BowTypeList`** — the set of bow-type view ids (affects ranged attack
  animation).

Add a `weapon` converter to `ro-to-lifthrasir-cli` mirroring
`ro-to-lifthrasir-cli/src/converters/accessory.rs`:

1. `lua::exec_chunk(&lua, weapontable.lub bytes)` (decompiling via `decompile.rs`
   if it starts with the Lua bytecode magic, exactly like `read_grf_lub`).
2. Read `WeaponNameTable`, `WeaponHitWaveNameTable`, and `BowTypeList` from the VM
   globals, decoding EUC-KR strings via the existing `decode_euckr` helper.
3. Serialize to `weapon_data.ron` under the same assets data dir the accessory
   converter writes to.

New `WeaponData` struct in `lifthrasir-data/src/lib.rs` (mirroring
`AccessoryData`):

```rust
pub struct WeaponData {
    /// weapon view id -> sprite suffix (leading `_` included)
    pub names: BTreeMap<u16, String>,
    /// weapon view id -> hit wav filename
    pub hit_sounds: BTreeMap<u16, String>,
    /// weapon view ids that are bow-type
    pub bow_types: BTreeSet<u16>,
}
```

New `WeaponDb` resource + plugin mirroring
`game-engine/src/infrastructure/accessory/{asset.rs,registry.rs,plugin.rs}`,
exposing `suffix(view_id) -> Option<&str>` and `hit_sound(view_id) -> Option<&str>`.

**Shields**: there is no `shieldtable.lub`. Shields are named
`<job>_<gender>_<suffix>_방패`, where the suffix is a classic class name for the
first views and the raw view id for renewal shields. Mirror roBrowser with a small
hardcoded classic table plus a numeric fallback, living next to the shield path
resolver (no CLI work, no RON):

```
1 -> 가드 (Guard)   2 -> 쉴드 (Shield)
3 -> 버클러 (Buckler) 4 -> 미러쉴드 (Mirror Shield)
n -> "<n>" (fallback for renewal shields, e.g. 28901)
```

## 3. Path resolution

Add builders to `game-engine/src/domain/assets/patterns.rs`. Both need the
character's **job sprite name** (from `JobSpriteRegistry`) + **gender** + the
resolved suffix (headgear only needed gender, so this is the new dependency):

- Weapon: `ro://data/sprite/인간족/{job}/{job}_{gender}{suffix}.spr` (+`.act`)
  e.g. `검사_남_1116.spr` (suffix `_1116` includes the leading `_`).
  Note the weapon lives under the **job** folder with no gender subfolder, unlike
  the body which is under `인간족/몸통/{gender}/`.
- Shield: `ro://data/sprite/방패/{job}/{job}_{gender}_{suffix}_방패.spr` (+`.act`)
  e.g. `검사_남_가드_방패.spr` or `검사_남_28901_방패.spr`.

`handle_equipment_changes`
(`game-engine/src/domain/entities/sprite_rendering/systems/events.rs`) gains a
job/appearance query (headgear didn't need one) and branches resolution per slot:
headgear → existing `resolve_headgear_paths`; weapon/shield → new resolvers.

Per the "critical systems fail loudly" rule, a non-zero view id that resolves to a
missing sprite is a loud failure. A `0` view id resolves to "no layer" and is not
an error.

## 4. Rendering — body-anchored sync

Add `game-engine/src/domain/entities/sprite_rendering/systems/weapon_sync.rs` with
`sync_weapon_layer` (handling both weapon and shield slots). Unlike
`sync_headgear_layer` (which rides the **head** anchor via `HeadAttachPoint`),
weapons and shields ride the **body** anchor. It therefore copies the body-anchor
math from `sync_player_head_layer` (`head_sync.rs`) reading `BodyAttachPoint`, not
the headgear math:

- Reuse `PlayerLayout::validate_action_index` for action/direction selection
  (weapon/shield ACTs share the player action indexing).
- Reuse the existing camera-rotated billboard-space delta and the per-layer
  z-offset push (`LAYER_WEAPON = 40` / `LAYER_SHIELD = 50` render over
  `LAYER_BODY = 20`).
- Register with `#[auto_add_system(... after = sync_player_body_layer)]` in
  `SpriteRenderingSystems::TransformUpdate`.

`finalize_equipment_layers` already spawns the layer child for these tags, so no
spawn-path changes are needed.

## 5. Event wiring (local + remote)

All three drivers emit the shared `EquipmentChangeEvent { character, slot,
view_id }`.

- **Remote spawn**: extend
  `game-engine/src/domain/entities/spawning/remote_headgear.rs` (or a sibling
  system) to emit weapon/shield change events from `EquipmentSet`, which
  `spawning/systems.rs` already populates from `UnitEntered.weapon/shield`.
- **Local player**: extend the `desired_*` reconciliation in
  `game-engine/src/domain/equipment/local_headgear.rs` to include weapon/shield
  from the `Inventory`'s equipped view ids.
- **Live changes**: extend `apply_sprite_changes`
  (`game-engine/src/domain/equipment/sprite_change.rs`) for **`LOOK_WEAPON = 2`**.
  This look type uniquely maps to **two** slots: `val` (weapon_view) and `val2`
  (shield_view) → emit one `EquipmentChangeEvent` per slot on **every** such
  change. A `0` view id is emitted too (not skipped): `handle_equipment_changes`
  despawns that slot's existing layer, which is exactly how an unequip
  (weapon or shield removed → `val`/`val2` = 0) despawns the sprite.

## 6. SFX — body-ACT sound (chosen), per-weapon-type table wired for follow-up

Verified against the GRF: **weapon ACTs contain zero sounds** (e.g.
`검사_남_1116.act` → 0 sound entries, no per-frame `sound_id`). The attack sound
lives in the **body** ACT (`검사_남.act` → sounds `["attack_sword.wav",
"attack_spear.wav", "player_clothes.wav"]`) referenced by per-frame `sound_id`.
Today `sync_player_body_layer`
(`game-engine/src/domain/entities/sprite_rendering/systems/body_sync.rs`) passes
`None` for the SFX writer, so player body sounds are silent (only
`sync_mob_body_layer` emits `PlayMobSfx`).

**Chosen approach (minimal):**

1. Give players a `SpatialAudioEmitter` at spawn (only mobs get one today;
   receiver `SpatialAudioReceiver` is already on the local player —
   `game-engine/src/domain/character/systems.rs`).
2. Pass the `PlayMobSfx` writer in `sync_player_body_layer` (flip the `None`),
   reusing the existing frame-crossing detection. The body ACT then fires its own
   attack sound. No weapon-layer SFX code.

**Deferred (fast follow-up, not in this spec):** the body ACT's fixed `sound_id`
does not vary the sound by equipped weapon type (sword vs axe). RO-accurate
per-weapon sound plays `WeaponHitWaveNameTable[view_id]` on the attack frame. §2
captures that table into `WeaponDb.hit_sounds` so this is a wired-but-unused table
ready for the follow-up; no rendering work depends on it.

## 7. Edge cases & scope boundaries

- **Two-handed weapons**: server zeroes `shield_view` → no shield layer. No client
  handling needed.
- **Bow type**: recorded in `WeaponDb.bow_types`; the ranged-attack body animation
  variant is **out of scope** for v1 (follow-up).
- **Riding / mounted** job sprites (`_riding`, `페코페코_기사`, etc.) exist in the
  GRF; **out of scope** for v1.
- **Weapon effect / glow overlay** (high-level weapon trails): **YAGNI**, excluded.
- **Equipment-window live preview**
  (`lifthrasir-ui/src/widgets/equipment_window/preview.rs`) renders the local
  player through the same pipeline; it will pick up weapons/shields once rendered,
  but its slot cache currently tracks only head slots — extending it to the two new
  slots is a small follow-up, not required for v1.

## 8. Testing

- `WeaponData` RON serialize/deserialize round-trip (mirror the existing
  `AccessoryData` test in `lifthrasir-data`).
- Converter: extract from `weapontable.lub`, assert non-zero `WeaponNameTable`,
  `WeaponHitWaveNameTable`, and `BowTypeList` counts, and spot-check a known entry
  (`2 -> _검`).
- Path builders: weapon (class suffix `_검` and item suffix `_1116`) and shield
  (classic `가드` and renewal `28901`) unit tests.
- `LOOK_WEAPON = 2` → two `EquipmentChangeEvent`s (weapon + shield) mapping test,
  including `val2 = 0` still emitting a shield event with `view_id = 0` (unequip
  despawns the layer).
- Manual: equip/unequip weapon and shield on the local player and on a remote
  player; verify layering (weapon/shield over body), correct sprite per job/gender,
  and that the attack sound plays.
