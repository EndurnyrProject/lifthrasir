use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// The server-authoritative outcome of an item-production attempt (forging,
/// brewing, ...). The produced item itself arrives separately as an inventory
/// update; this message only carries the outcome for player feedback.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ProductionResult {
    /// Whether the attempt produced the item.
    pub success: bool,
    /// The recipe's product item id, on either outcome.
    pub item_id: u32,
}
