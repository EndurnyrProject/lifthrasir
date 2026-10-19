//! Protocol-neutral NPC shop types.

/// One item the shop NPC has for sale; `nameid` resolves via `ItemDb` client-side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopBuyItem {
    pub nameid: u32,
    pub price: u32,
}

/// One of the local player's own items the shop will buy back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopSellItem {
    pub inventory_index: u32,
    pub nameid: u32,
    pub amount: u32,
    pub sell_price: u32,
}

/// One buy-cart line: `nameid` and how many to purchase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyEntry {
    pub nameid: u32,
    pub amount: u32,
}

/// One sell-cart line: `inventory_index` and how many to sell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellEntry {
    pub inventory_index: u32,
    pub amount: u32,
}

/// The server's result code for a buy or sell request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopResult {
    Ok,
    NotEnoughZeny,
    Overweight,
    NoSlots,
    Invalid,
    OutOfRange,
}
