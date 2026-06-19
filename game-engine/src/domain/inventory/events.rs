use super::item::Item;
use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct InventoryDumpStarted;

#[derive(Message, Debug, Clone)]
pub struct InventoryItemsReceived {
    pub items: Vec<Item>,
}

#[derive(Message, Debug, Clone)]
pub struct InventoryDumpCompleted;
