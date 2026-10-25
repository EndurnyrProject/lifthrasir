//! Protocol-neutral setup grants, shop instances and vending outcomes.

use super::ItemOption;

/// One cart instance offered by its owner, using the ordinary neutral cart index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingOffer {
    pub cart_index: u32,
    pub amount: u32,
    pub price: u32,
}

/// One checkout line against an opaque listing index and its displayed price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingBuyLine {
    pub index: u32,
    pub amount: u32,
    pub expected_unit_price: u32,
}

/// The exact preparation or shop instance a close request may affect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendingCloseTarget {
    Setup(String),
    Shop(String),
}

/// Item provenance, preserving unrecognized values without changing their meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingCreatorKind {
    None,
    Signed,
    Forged,
    Unknown(i32),
}

/// A particular item instance on display, not a catalog-only NPC shop item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingListing {
    /// Opaque listing identity; never interpret this as a neutral cart index.
    pub index: u32,
    pub nameid: u32,
    pub type_: u32,
    pub amount: u32,
    pub identified: bool,
    pub attribute: u32,
    pub refine: u32,
    pub cards: Vec<u32>,
    pub options: Vec<ItemOption>,
    pub price: u32,
    pub signer_name: String,
    pub creator_id: u32,
    pub creator_kind: VendingCreatorKind,
    pub enchant_grade: u32,
}

/// Server-approved preparation after the skill's normal cost has been paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingSetup {
    pub setup_id: String,
    pub max_slots: u32,
    pub min_unit_price: u32,
    pub max_unit_price: u32,
    pub max_title_bytes: u32,
    /// Eligible slots in the same index space as `CartItem::index`.
    pub sellable_cart_indices: Vec<u32>,
    pub sp_spent: u32,
}

/// An authoritative snapshot belonging to one particular opening of a shop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingSnapshot {
    pub vendor_unit_id: u64,
    pub shop_id: String,
    pub title: String,
    pub items: Vec<VendingListing>,
    /// Private cumulative net credit; remote browse snapshots carry zero.
    pub total_earned: u64,
}

/// A visible vendor's board, independent of whether its entity has spawned yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingBoard {
    pub vendor_unit_id: u64,
    pub shop_id: String,
    pub title: String,
}

/// Outcome of submitting listings against an existing setup grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingOpenOutcome {
    Success,
    NoCart,
    SkillNotLearned,
    TooManySlots,
    InvalidAmount,
    InvalidPrice,
    ItemNotInCart,
    InsufficientStock,
    InvalidState,
    InsufficientSp,
    NpcNearby,
    MapRestricted,
    CellRestricted,
    InvalidTitle,
    NoItems,
    DuplicateSlot,
    ItemUnidentified,
    ItemBroken,
    ItemRental,
    ItemBound,
    ItemUntradeable,
    SetupExpired,
    Forbidden,
    ZenyLimit,
    PersistenceFailed,
    InvalidRequest,
    Unknown(i32),
}

/// Browse outcome; an unknown or unspecified value never means success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingListOutcome {
    Success,
    NotOpen,
    ShopChanged,
    Forbidden,
    InvalidRequest,
    InternalError,
    Unknown(i32),
}

/// Definitive outcome of an atomic checkout, not an inventory mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingPurchaseOutcome {
    Success,
    NotOpen,
    ShopChanged,
    PriceChanged,
    InsufficientStock,
    InvalidLine,
    InvalidAmount,
    NotEnoughZeny,
    Overweight,
    NoSlots,
    StackLimit,
    SellerZenyLimit,
    Forbidden,
    OutOfRange,
    WrongMap,
    SelfPurchase,
    InvalidRequest,
    PersistenceFailed,
    InternalError,
    StaleRequest,
    InvalidState,
    Unknown(i32),
}

/// Acknowledgement of canceling preparation or explicitly closing a shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingCloseOutcome {
    ShopClosed,
    SetupCanceled,
    AlreadyClosed,
    InvalidTarget,
    Forbidden,
    InvalidRequest,
    InternalError,
    Unknown(i32),
}

/// Why an identified shop board was removed, distinct from visibility loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingClosureReason {
    UserClosed,
    SoldOut,
    Disconnected,
    JobChanged,
    Relocated,
    Invalidated,
    Unknown(i32),
}
