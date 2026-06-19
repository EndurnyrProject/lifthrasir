#[derive(Debug, Clone, Default)]
pub struct Item {
    /// Client index (already +2 from server) — the map key.
    pub index: u16,
    /// Server nameid.
    pub item_id: u32,
    /// Client IT_* enum, raw.
    pub item_type: u8,
    /// Equippables → 1.
    pub amount: u16,
    /// Allowed equip slots (stackables → 0).
    pub location: u32,
    /// Worn bitmask (0 = in bag).
    pub wear_state: u32,
    /// Stackables → 0.
    pub refine: u8,
    pub cards: [u32; 4],
    pub options: Vec<ItemOption>,
    pub expire_time: u32,
    /// Equip 'sprite' view id; stackables → 0.
    pub view_sprite: u16,
    pub identified: bool,
    pub damaged: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ItemOption {
    pub index: u16,
    pub value: u16,
    pub param: u8,
}

impl Item {
    pub fn is_equipped(&self) -> bool {
        self.wear_state != 0
    }
}
