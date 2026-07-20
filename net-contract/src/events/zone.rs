use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::dto::{SkillUnitDespawnReason, SkillUnitGroupState, SkillUnitUpdateReason};

/// Zone entry accepted: AID folded into EnterAck (collapses ZC_ACCEPT_ENTER + ZC_AID).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ZoneEntered {
    pub account_id: u32,
    pub x: u32,
    pub y: u32,
    pub dir: u32,
    pub start_time: u64,
}

/// Own-character authoritative movement.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SelfMoved {
    pub src_x: u32,
    pub src_y: u32,
    pub dst_x: u32,
    pub dst_y: u32,
    pub start_time: u64,
}

/// Server commanded a map change (warp); the client unloads, loads `map_name`, and re-places at (x, y).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct MapChangeRequested {
    pub map_name: String,
    pub x: u32,
    pub y: u32,
}

/// An entity stopped moving.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitMoveStopped {
    pub gid: u32,
    pub x: u32,
    pub y: u32,
}

/// An entity entered view (collapses new/stand/move-entry; move fields carry the moving case).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitEntered {
    pub gid: u32,
    pub aid: u32,
    pub object_type: u32,
    pub job: u32,
    pub x: u32,
    pub y: u32,
    pub dir: u32,
    pub speed: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub clevel: u32,
    pub body_state: u32,
    pub health_state: u32,
    pub effect_state: u32,
    pub head: u32,
    pub weapon: u32,
    pub shield: u32,
    pub accessory: u32,
    pub accessory2: u32,
    pub accessory3: u32,
    pub head_palette: u32,
    pub body_palette: u32,
    pub head_dir: u32,
    pub robe: u32,
    pub guild_id: u32,
    pub guild_name: String,
    pub emblem_id: u32,
    pub sex: u32,
    pub is_boss: bool,
    pub name: String,
    pub moving: bool,
    pub dst_x: u32,
    pub dst_y: u32,
    pub move_start_time: u64,
}

/// An entity left view.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitLeft {
    pub gid: u32,
    pub reason: u32,
}

/// An entity's name (collapses ZC_ACK_REQNAME + ZC_ACK_REQNAMEALL).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct EntityNamed {
    pub gid: u32,
    pub name: String,
    pub party_name: String,
    pub guild_name: String,
    pub position_name: String,
}

/// An entity's chat message.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ChatHeard {
    pub gid: u32,
    pub message: String,
}

/// An entity performed an emote.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct EmoteShown {
    pub gid: u32,
    pub emote_type: u32,
}

/// Basic-attack damage result; `damage`/`damage2` stay signed.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct DamageReceived {
    pub src_id: u32,
    pub target_id: u32,
    pub server_tick: u64,
    pub src_speed: u32,
    pub dmg_speed: u32,
    pub damage: i32,
    pub div: u32,
    pub type_: u32,
    pub damage2: i32,
}

/// An entity's HP changed.
// NOTE: no client consumer yet; kept for future implementation.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitHpChanged {
    pub gid: u32,
    pub hp: u32,
    pub max_hp: u32,
}

/// A status effect (EFST icon) was applied (`on = true`) or removed on a unit.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StatusEffectChanged {
    pub unit_id: u32,
    pub efst: u32,
    pub on: bool,
    /// Total duration in ms as sent by the server; `0` = infinite/permanent.
    pub total_ms: u32,
    /// Remaining duration in ms at the time of this event; `0` = infinite/permanent.
    pub remain_ms: u32,
}

/// A fire-and-forget visual effect (rAthena `EF_*` id) triggered by `source_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SpecialEffectShown {
    pub source_id: u32,
    pub effect_id: u32,
}

/// Legacy unit-state flags (opt1/opt2/option/opt3): stone/freeze/stun/sleep
/// poses, poison/curse/silence, hide/cloak/mount, and virtue. A separate
/// channel from the EFST `StatusEffectChanged`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitStateChanged {
    pub unit_id: u32,
    pub body_state: u32,
    pub health_state: u32,
    pub effect_state: u32,
    pub virtue: u32,
}

/// A parameter changed (collapses ZC_PAR_CHANGE u16 + ZC_LONGPAR_CHANGE u32).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ParamChanged {
    pub var: u32,
    pub value: u64,
}

/// One inventory slot, mirroring the proto InventoryList item shape.
#[derive(Debug, Clone)]
pub struct ZoneInventoryItem {
    pub index: u32,
    pub nameid: u32,
    pub type_: u32,
    pub amount: u32,
    pub location: u32,
    pub identified: bool,
    pub attribute: u32,
    pub refine: u32,
    pub cards: Vec<u32>,
    pub expire_time: u64,
    pub bind_on_equip: u32,
    pub favorite: bool,
    pub look: u32,
}

/// The full inventory dump (collapses ZC_INVENTORY_START/ITEMLIST_*/END).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct InventoryReceived {
    pub items: Vec<ZoneInventoryItem>,
}

/// A skill cast bar started.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillCastStarted {
    pub src_id: u32,
    pub target_id: u32,
    pub x: u32,
    pub y: u32,
    pub skill_id: u32,
    pub property: u32,
    pub cast_time: u32,
}

/// Skill damage result.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillDamageReceived {
    pub skill_id: u32,
    pub level: u32,
    pub src_id: u32,
    pub target_id: u32,
    pub server_tick: u64,
    pub damage: i32,
    pub div: u32,
    pub type_: u32,
    pub src_delay: u32,
    pub dst_delay: u32,
}

/// No-damage skill effect.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillEffectShown {
    pub skill_id: u32,
    pub level: u32,
    pub src_id: u32,
    pub target_id: u32,
    pub result: u32,
}

/// A skill cast was cancelled.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CastCancelled {
    pub gid: u32,
}

/// Full skill-unit state for the zone, sent on zone-in (e.g. Storm Gust
/// groups already active on the map).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillUnitSnapshotReceived {
    pub server_tick: u64,
    pub groups: Vec<SkillUnitGroupState>,
}

/// A skill-unit group was placed (e.g. a Storm Gust cast, an Ice Wall).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillUnitSpawned {
    pub group: SkillUnitGroupState,
}

/// A skill-unit cell's HP changed.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillUnitUpdated {
    pub group_id: u64,
    pub cell_id: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub hp_delta: i32,
    pub reason: SkillUnitUpdateReason,
}

/// One or more cells of a skill-unit group were removed; the group root
/// despawns once its last cell is gone.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillUnitDespawned {
    pub group_id: u64,
    pub cell_ids: Vec<u32>,
    pub reason: SkillUnitDespawnReason,
}

/// A skill went on cooldown.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillCooldownSet {
    pub skill_id: u32,
    pub tick: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillCastFailureReason {
    Unspecified,
    MissingCatalyst,
    InsufficientSp,
    InsufficientZeny,
    NoAmmo,
    OnCooldown,
    InvalidTarget,
    NotLearned,
    OutOfRange,
    Busy,
}

/// A player-initiated skill cast was rejected by the server.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillCastFailed {
    pub skill_id: u32,
    pub reason: SkillCastFailureReason,
}

/// One learned skill within a SkillListReceived.
#[derive(Debug, Clone)]
pub struct ZoneSkillInfo {
    pub skill_id: u32,
    pub type_: u32,
    pub level: u32,
    pub sp: u32,
    pub range: u32,
    pub name: String,
    pub upgradable: bool,
    pub max_level: u32,
    /// Prerequisite skills as `(skill_id, level)` pairs.
    pub requires: Vec<(u32, u32)>,
    pub req_base_level: u32,
    pub req_job_level: u32,
    pub job_id: u32,
    pub splash_radius: u16,
}

/// The full learned-skill list.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillListReceived {
    pub skills: Vec<ZoneSkillInfo>,
}

/// The result of a learn attempt.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct LearnSkillResultReceived {
    pub skill_id: u32,
    pub ok: bool,
    pub reason: u32,
}

/// A ground-targeted skill landed.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct GroundSkillPlaced {
    pub skill_id: u32,
    pub src_id: u32,
    pub level: u32,
    pub x: u32,
    pub y: u32,
    pub server_tick: u64,
}

/// An entity was knocked back.
// NOTE: no client consumer yet; kept for future implementation.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct KnockedBack {
    pub unit_id: u32,
    pub dst_x: u32,
    pub dst_y: u32,
}

/// An item was added to the inventory.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemAdded {
    pub index: u32,
    pub amount: u32,
    pub nameid: u32,
    pub identified: bool,
    pub attribute: u32,
    pub refine: u32,
    pub cards: Vec<u32>,
    pub location: u32,
    pub type_: u32,
    pub result: u32,
    pub expire_time: u64,
    pub look: u32,
}

/// An item appeared on the ground.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemOnGround {
    pub ground_id: u64,
    pub nameid: u32,
    pub amount: u32,
    pub x: u16,
    pub y: u16,
    pub identified: bool,
    pub is_falling: bool,
    pub sub_x: u8,
    pub sub_y: u8,
}

/// A ground item was removed from view.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemVanished {
    pub ground_id: u64,
    pub reason: VanishReason,
}

/// Result of a pickup request.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PickupResult {
    pub ground_id: u64,
    pub outcome: PickupOutcome,
}

/// Why a ground item vanished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VanishReason {
    PickedUp,
    Expired,
}

/// Outcome of a pickup attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupOutcome {
    Ok,
    TooFar,
    Overweight,
    InventoryFull,
    Gone,
    Failed,
}

/// An item was removed from the inventory.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemRemoved {
    pub index: u32,
    pub amount: u32,
    pub reason: u32,
}

/// Result of an equip request.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemEquipped {
    pub index: u32,
    pub wear_location: u32,
    pub view_id: u32,
    pub result: u32,
}

/// Result of an unequip request.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemUnequipped {
    pub index: u32,
    pub wear_location: u32,
    pub result: u32,
}

/// A use-item attempt was rejected; carries the server reason code.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemUseFailed {
    pub index: u32,
    pub reason: u32,
}

/// Result of a stat allocation.
// NOTE: no client consumer yet; kept for future implementation.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StatRaised {
    pub stat_id: u32,
    pub ok: bool,
    pub value: u32,
}

/// An entity's sprite changed.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitSpriteChanged {
    pub gid: u32,
    pub type_: u32,
    pub val: u32,
    pub val2: u32,
}

/// An entity was resurrected.
// NOTE: no client consumer yet; kept for future implementation.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnitResurrected {
    pub gid: u32,
    pub type_: u32,
}

/// Self respawn / return to char select after death.
// NOTE: never emitted — respawn() is not wired into any drainer; game-engine reader is waiting on this.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SelfRespawned {
    pub type_: u32,
}

/// The zone connection was lost or refused.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ZoneDisconnected {
    pub reason: String,
}

/// One entity's authoritative position/state within a snapshot.
#[derive(Debug, Clone, Copy)]
pub struct ZoneSnapshotEntity {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub dir: u32,
    pub move_state: u32,
    pub hp_pct: u32,
}

/// A periodic full-state position snapshot (unreliable channel).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SnapshotReceived {
    pub server_tick: u64,
    pub entities: Vec<ZoneSnapshotEntity>,
}
