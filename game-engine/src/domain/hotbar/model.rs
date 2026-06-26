use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum HotbarSlot {
    Skill(u32),
    Item(u32),
}

#[derive(Resource, Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[auto_init_resource(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct Hotbar {
    pub slots: [Option<HotbarSlot>; 12],
}

impl Hotbar {
    pub fn get(&self, i: usize) -> Option<HotbarSlot> {
        self.slots.get(i).copied().flatten()
    }

    pub fn assign(&mut self, i: usize, slot: HotbarSlot) {
        if let Some(s) = self.slots.get_mut(i) {
            *s = Some(slot);
        }
    }

    pub fn clear(&mut self, i: usize) {
        if let Some(s) = self.slots.get_mut(i) {
            *s = None;
        }
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() {
            self.slots.swap(a, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assign_overwrites_slot() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(10));
        assert_eq!(bar.get(0), Some(HotbarSlot::Skill(10)));
        bar.assign(0, HotbarSlot::Item(99));
        assert_eq!(bar.get(0), Some(HotbarSlot::Item(99)));
    }

    #[test]
    fn clear_empties_slot() {
        let mut bar = Hotbar::default();
        bar.assign(3, HotbarSlot::Skill(5));
        bar.clear(3);
        assert_eq!(bar.get(3), None);
    }

    #[test]
    fn swap_exchanges_slots() {
        let mut bar = Hotbar::default();
        bar.assign(1, HotbarSlot::Skill(7));
        bar.assign(2, HotbarSlot::Item(42));
        bar.swap(1, 2);
        assert_eq!(bar.get(1), Some(HotbarSlot::Item(42)));
        assert_eq!(bar.get(2), Some(HotbarSlot::Skill(7)));
    }

    #[test]
    fn swap_with_one_empty() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        bar.swap(0, 5);
        assert_eq!(bar.get(0), None);
        assert_eq!(bar.get(5), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn out_of_range_get_returns_none() {
        let bar = Hotbar::default();
        assert_eq!(bar.get(12), None);
        assert_eq!(bar.get(usize::MAX), None);
    }

    #[test]
    fn out_of_range_assign_is_noop() {
        let mut bar = Hotbar::default();
        bar.assign(12, HotbarSlot::Skill(1));
        assert!(bar.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn out_of_range_clear_is_noop() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        bar.clear(12);
        assert_eq!(bar.get(0), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn out_of_range_swap_is_noop() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        bar.swap(0, 12);
        assert_eq!(bar.get(0), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn ron_round_trip_populated_bar() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(40));
        bar.assign(2, HotbarSlot::Item(501));
        bar.assign(11, HotbarSlot::Skill(100));

        let serialized = ron::to_string(&bar).expect("serialize");
        let restored: Hotbar = ron::from_str(&serialized).expect("deserialize");
        assert_eq!(bar, restored);
    }
}
