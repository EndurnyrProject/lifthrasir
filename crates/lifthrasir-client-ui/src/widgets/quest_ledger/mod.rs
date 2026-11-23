//! The quest ledger window and tracker plate. This module holds the UI state;
//! [`view`] turns the quest log into what the scene displays.

pub mod view;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

/// Which list of quests the ledger shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LedgerTab {
    #[default]
    Active,
    Shelved,
    Completed,
}

/// What the player has done in the ledger window.
#[derive(Resource, Default, Debug)]
pub struct LedgerUi {
    pub open: bool,
    pub tab: LedgerTab,
    pub selected: Option<u32>,
    pub query: String,
    /// A message and the seconds it has left on screen.
    pub toast: Option<(String, f32)>,
}
