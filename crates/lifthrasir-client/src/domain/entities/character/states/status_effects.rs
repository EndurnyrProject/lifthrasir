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
    pub fn clear_all(&mut self) {
        *self = Self::default();
    }
}
