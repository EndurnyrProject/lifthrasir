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

/// Why the server rejected a cart mount attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartMountRejection {
    /// The character has not learned `MC_PUSHCART`.
    SkillNotLearned,
    /// A cart is already mounted.
    AlreadyMounted,
}

/// The server's outcome of a [`MountCart`](crate::commands::MountCart) request.
/// A successful mount is `Ok`; the cart sprite and `CartLoaded` already reflect
/// it, so the UI only surfaces the rejection reason. Only mount (not unmount)
/// attempts produce this.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CartMountResult {
    pub outcome: Result<(), CartMountRejection>,
}
