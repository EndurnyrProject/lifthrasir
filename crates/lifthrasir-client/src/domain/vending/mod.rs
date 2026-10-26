//! Online vending: seller setup and monitoring, buyer browsing, and checkout.

pub mod plugin;
pub mod state;
pub mod systems;

#[cfg(test)]
mod tests;

pub use plugin::{VendingPlugin, VendingSystems};
pub use state::{
    MUTATION_TIMEOUT_SECS, RECENT_SALES, RevealVendingSeller, VendingIntent, VendingNotice,
    VendingRefresh, VendingSale, VendingState,
};
pub use systems::VENDING_SKILL_ID;
