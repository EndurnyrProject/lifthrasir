use bevy::prelude::*;

#[derive(Clone, Debug, Default, Component, Reflect)]
pub struct StatusEffects {
    pub stunned: bool,
    pub frozen: bool,
    pub petrified: bool,
    pub poisoned: bool,
    pub sleeping: bool,
    pub bleeding: bool,
    pub hiding: bool,
    pub cloaking: bool,
    pub dead: bool,
}

impl StatusEffects {
    pub fn is_incapacitated(&self) -> bool {
        self.stunned || self.frozen || self.petrified || self.sleeping || self.dead
    }

    pub fn is_alive(&self) -> bool {
        !self.dead
    }

    pub fn clear_all(&mut self) {
        *self = Self::default();
    }
}
