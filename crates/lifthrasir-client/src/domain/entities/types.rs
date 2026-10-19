/// Type of game object/entity
///
/// Represents the different types of entities that can exist in the game world.
/// These values map to the server protocol's entity type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    /// Player character
    Pc = 0x0,
    /// Non-player character (NPC)
    Npc = 0x1,
    /// Monster/mob
    Mob = 0x5,
    /// Homunculus (player's summoned creature)
    Homunculus = 0x6,
    /// Mercenary (hired fighter)
    Mercenary = 0x7,
    /// Elemental (summoned element)
    Elemental = 0x8,
    /// Server-authoritative skill-unit cell (Ice Wall, Fire Pillar, ...).
    /// Client-spawned only: never parsed off the wire, so `From<u8>` has no
    /// arm producing it (unknown wire values fall back to `Pc`).
    SkillUnit = 0xFF,
}

impl From<u8> for ObjectType {
    fn from(value: u8) -> Self {
        match value {
            0x0 => ObjectType::Pc,
            0x1 => ObjectType::Npc,
            0x5 => ObjectType::Mob,
            0x6 => ObjectType::Homunculus,
            0x7 => ObjectType::Mercenary,
            0x8 => ObjectType::Elemental,
            _ => ObjectType::Pc, // Default to PC for unknown types
        }
    }
}

impl From<ObjectType> for u8 {
    fn from(value: ObjectType) -> Self {
        value as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_parsing_never_produces_skill_unit() {
        for value in 0..=u8::MAX {
            assert_ne!(
                ObjectType::from(value),
                ObjectType::SkillUnit,
                "wire value {value} must not decode to the client-only SkillUnit variant"
            );
        }
    }
}
