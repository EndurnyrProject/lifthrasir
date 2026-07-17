use bevy::prelude::*;
use net_contract::dto::SkillUnitCellFlags;

/// Root entity of a server-authoritative ground-skill unit group (one Storm Gust
/// cast, one Ice Wall, ...). Its `Transform` sits at the group center; cells are
/// children. Carries `MapScoped` so it dies on zone change. Despawning it is
/// recursive, tearing down every cell and attached visual with it.
#[derive(Component, Debug)]
pub struct SkillUnitGroup {
    pub group_id: u64,
    pub skill_id: u32,
    pub level: u32,
    pub owner_id: u32,
}

/// One cell of a group (one occupied tile). A child of the group root, positioned
/// relative to the root so its world transform lands on the cell. HP is kept
/// server-authoritative for future use (no HP bar in scope).
#[derive(Component, Debug)]
pub struct SkillUnitCell {
    pub group_id: u64,
    pub cell_id: u32,
    pub flags: SkillUnitCellFlags,
    pub hp: u32,
    pub max_hp: u32,
}
