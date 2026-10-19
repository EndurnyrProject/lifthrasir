use crate::{dto::TradeCancelReason, events::zone::ZoneInventoryItem};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Server-to-client notification that another player wants to trade.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct TradeRequestNotified {
    pub char_id: u32,
    pub name: String,
}

/// Server-to-client notification that a trade has opened.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct TradeOpened {
    pub partner_char_id: u32,
    pub partner_name: String,
}

/// Server-to-client authoritative offer snapshot. Partner entries carry `index == 0`.
#[derive(Message, Debug, Clone, PartialEq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct TradeOfferUpdated {
    pub own: Vec<ZoneInventoryItem>,
    pub partner: Vec<ZoneInventoryItem>,
    pub own_zeny: u64,
    pub partner_zeny: u64,
    pub own_locked: bool,
    pub partner_locked: bool,
}

/// Server-to-client notification that both players completed the trade.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct TradeCompleted;

/// Server-to-client notification that a trade request or open trade ended.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct TradeCancelled {
    pub reason: TradeCancelReason,
}
