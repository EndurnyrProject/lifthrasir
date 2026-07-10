use crate::dto::CartItem;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// The full cart dump, sent on mount or login (mirrors `InventoryReceived`).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CartLoaded {
    pub items: Vec<CartItem>,
    pub current_weight: u32,
    pub max_weight: u32,
}

/// An item was added to the cart.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CartItemAdded {
    pub item: CartItem,
}

/// An item was removed from the cart.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CartItemRemoved {
    pub index: u16,
    pub amount: u16,
    pub reason: u32,
}
