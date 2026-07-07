use crate::dto::{ShopBuyItem, ShopResult, ShopSellItem};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// A shop NPC opened its buy/sell window for the local player.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ShopOpened {
    pub unit_id: u64,
    pub buy_items: Vec<ShopBuyItem>,
    pub sell_items: Vec<ShopSellItem>,
}

/// Result of a batched buy request.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ShopBuyResulted {
    pub result: ShopResult,
}

/// Result of a batched sell request.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ShopSellResulted {
    pub result: ShopResult,
}
