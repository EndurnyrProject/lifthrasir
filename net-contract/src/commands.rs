//! Outbound command Messages (client to server).

use crate::dto::{BuyEntry, NpcResponse, SellEntry};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Request to move the local player to a destination cell.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct MoveRequested {
    pub dest_x: u16,
    pub dest_y: u16,
}

/// Request to equip the inventory item at `index` to its worn `location` mask.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct EquipRequested {
    pub index: u16,
    pub location: u32,
}

/// Request to unequip the inventory item at `index`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnequipRequested {
    pub index: u16,
}

/// Request to use (consume) the inventory item at `index`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UseRequested {
    pub index: u32,
}

/// Request to pick up the ground item identified by `ground_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PickupRequested {
    pub ground_id: u64,
}

/// Request to send a chat line; `message` is the wire-ready, formatted string.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ChatSent {
    pub message: String,
}

/// Request to perform an emote identified by `emote_type`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct EmoteSent {
    pub emote_type: u32,
}

/// Request to cast a single-target skill (`skill_id` at `level`) at `target_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillCastRequested {
    pub skill_id: u32,
    pub level: u32,
    pub target_id: u32,
}

/// Request to cast a ground-targeted skill (`skill_id` at `level`) on cell (`x`, `y`).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct GroundSkillCastRequested {
    pub skill_id: u32,
    pub level: u32,
    pub x: u32,
    pub y: u32,
}

/// Request a basic attack against the entity identified by `target_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct AttackRequested {
    pub target_id: u32,
}

/// Request to sit (`sit == true`) or stand (`sit == false`) the local player.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SitToggled {
    pub sit: bool,
}

/// Request to allocate `amount` points into the stat identified by `stat_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StatRaiseRequested {
    pub stat_id: u32,
    pub amount: u32,
}

/// Request to learn (raise a level of) the skill identified by `skill_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct LearnSkillRequested {
    pub skill_id: u32,
}

/// Request the display name of the entity identified by `gid`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct NameRequested {
    pub gid: u32,
}

/// Request to select the character in the given char-list `slot`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SelectCharacter {
    pub slot: u32,
}

/// Request to create a character; the domain flattens and validates the form first.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CreateCharacter {
    pub name: String,
    pub slot: u32,
    pub hair_color: u32,
    pub hair_style: u32,
    pub starting_job: u32,
    pub sex: u32,
}

/// Request to delete the character identified by `char_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct DeleteCharacter {
    pub char_id: u32,
}

/// Request a fresh character list from the char server.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct RefreshCharacterList;

/// Request to open the login-server connection and begin the login handshake.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ConnectLogin {
    pub address: String,
    pub username: String,
    pub password: String,
    pub client_version: u32,
    pub build: String,
}

/// Request to open the char-server connection and begin the char-session handshake.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ConnectCharServer {
    pub address: String,
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: u32,
}

/// Request to open the zone-server connection and begin the zone handshake.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ConnectZone {
    pub address: String,
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: u32,
    pub char_id: u32,
    pub zone_auth_token: Vec<u8>,
    pub map_name: String,
}

/// Request to abandon the active zone session (e.g. on return to the login screen).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct LeaveZone;

/// Domain to adapter readiness signal: the local map asset finished loading.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct LocalMapLoaded;

/// Domain to adapter readiness signal: the local player entity exists.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct LocalPlayerReady;

/// Request to start an NPC dialogue with the NPC identified by `npc_id` (gid).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct TalkToNpc {
    pub npc_id: u32,
}

/// Respond to the active NPC dialogue frame for `npc_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct RespondToNpc {
    pub npc_id: u32,
    pub response: NpcResponse,
}

/// Request to buy `items` from the shop NPC identified by `unit_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct BuyFromShop {
    pub unit_id: u64,
    pub items: Vec<BuyEntry>,
}

/// Request to sell `items` to the shop NPC identified by `unit_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SellToShop {
    pub unit_id: u64,
    pub items: Vec<SellEntry>,
}

/// Request to respawn after death: `type_ == 0` at the save point, `type_ == 1` to char select.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct RespawnRequested {
    pub type_: u32,
}

/// Request to mount (`mount == true`) or unmount the pushcart.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct MountCart {
    pub mount: bool,
}

/// Request to move `amount` of the inventory item at `inventory_index` into the cart.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct MoveToCart {
    pub inventory_index: u16,
    pub amount: u16,
}

/// Request to move `amount` of the cart item at `cart_index` back into the inventory.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct MoveFromCart {
    pub cart_index: u16,
    pub amount: u16,
}

/// Request to create a party named `name`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyCreateRequested {
    pub name: String,
}

/// Request to invite a player to the local player's party; exactly one of
/// `target_char_id` / `target_name` is populated, the other left `0` / `""`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyInviteRequested {
    pub target_char_id: u32,
    pub target_name: String,
}

/// Respond to a pending invite for `party_id`: accept or decline.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyInviteResponded {
    pub party_id: u32,
    pub accept: bool,
}

/// Request to leave the local player's current party.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyLeaveRequested;
