//! Protocol-neutral NPC dialogue types.

/// What kind of response the server expects for the current NPC dialogue frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcDialogExpect {
    Next,
    Menu,
    InputInt,
    InputStr,
    Close,
}

/// The player's response to an NPC dialogue frame.
#[derive(Debug, Clone, PartialEq)]
pub enum NpcResponse {
    Continue,
    /// Selected menu option; **1-based** (the server expects 1-based menu indices).
    Choice(u32),
    Number(i64),
    Input(String),
    Cancel,
}
