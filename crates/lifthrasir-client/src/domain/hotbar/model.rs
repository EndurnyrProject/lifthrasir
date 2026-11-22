use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum HotbarSlot {
    Skill(u32),
    Item(u32),
    HomunculusSkill(u32),
}

/// Slots on one page of the bar.
pub const SLOTS: usize = 12;
/// Pages the bar can be turned through.
pub const PAGES: usize = 3;

/// One page of the bar.
pub type Page = [Option<HotbarSlot>; SLOTS];

/// The quick-bar: [`PAGES`] pages of [`SLOTS`] slots, and how it is shown. A slot is
/// addressed across pages as `page * SLOTS + column`.
#[derive(Resource, Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
#[auto_init_resource(plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin)]
pub struct Hotbar {
    pub pages: [Page; PAGES],
    /// The page on the first row.
    pub page: usize,
    /// Whether the following page shows as a second row.
    pub two_rows: bool,
    /// Whether the slots refuse to be moved, replaced or removed.
    pub locked: bool,
}

impl Hotbar {
    fn slot_mut(&mut self, i: usize) -> Option<&mut Option<HotbarSlot>> {
        self.pages.get_mut(i / SLOTS)?.get_mut(i % SLOTS)
    }

    pub fn get(&self, i: usize) -> Option<HotbarSlot> {
        self.pages.get(i / SLOTS)?[i % SLOTS]
    }

    pub fn assign(&mut self, i: usize, slot: HotbarSlot) {
        if let Some(s) = self.slot_mut(i) {
            *s = Some(slot);
        }
    }

    pub fn clear(&mut self, i: usize) {
        if let Some(s) = self.slot_mut(i) {
            *s = None;
        }
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        if a.max(b) >= SLOTS * PAGES {
            return;
        }
        let (was_a, was_b) = (self.get(a), self.get(b));
        self.pages[a / SLOTS][a % SLOTS] = was_b;
        self.pages[b / SLOTS][b % SLOTS] = was_a;
    }

    /// Whether `slot` sits anywhere on the bar.
    pub fn contains(&self, slot: HotbarSlot) -> bool {
        self.pages.as_flattened().contains(&Some(slot))
    }

    /// How many rows are on show.
    pub fn rows(&self) -> usize {
        if self.two_rows { 2 } else { 1 }
    }

    /// The page shown on `row`. A saved `page` past the last one wraps.
    pub fn row_page(&self, row: usize) -> usize {
        (self.page + row) % PAGES
    }

    /// The slot at `column` of the shown `row`.
    pub fn index(&self, row: usize, column: usize) -> usize {
        self.row_page(row) * SLOTS + column
    }

    /// Turns to the next page, or to the previous one, wrapping around.
    pub fn turn(&mut self, forward: bool) {
        let step = if forward { 1 } else { PAGES - 1 };
        self.page = (self.page + step) % PAGES;
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

    const PAST_END: usize = SLOTS * PAGES;

    #[test]
    fn out_of_range_get_returns_none() {
        let bar = Hotbar::default();
        assert_eq!(bar.get(PAST_END), None);
        assert_eq!(bar.get(usize::MAX), None);
    }

    #[test]
    fn out_of_range_assign_is_noop() {
        let mut bar = Hotbar::default();
        bar.assign(PAST_END, HotbarSlot::Skill(1));
        assert_eq!(bar, Hotbar::default());
    }

    #[test]
    fn out_of_range_clear_is_noop() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        bar.clear(PAST_END);
        assert_eq!(bar.get(0), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn out_of_range_swap_is_noop() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        bar.swap(0, PAST_END);
        assert_eq!(bar.get(0), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn slots_are_addressed_across_pages() {
        let mut bar = Hotbar::default();
        bar.assign(SLOTS + 2, HotbarSlot::Skill(7));
        assert_eq!(bar.pages[1][2], Some(HotbarSlot::Skill(7)));
        assert!(bar.contains(HotbarSlot::Skill(7)));
        assert!(!bar.contains(HotbarSlot::Item(7)));

        bar.swap(SLOTS + 2, 2 * SLOTS);
        assert_eq!(bar.get(SLOTS + 2), None);
        assert_eq!(bar.pages[2][0], Some(HotbarSlot::Skill(7)));
    }

    #[test]
    fn rows_follow_the_page_on_show() {
        let mut bar = Hotbar::default();
        assert_eq!((bar.rows(), bar.index(0, 3)), (1, 3));

        bar.two_rows = true;
        bar.turn(false);
        assert_eq!(bar.page, PAGES - 1);
        assert_eq!(bar.rows(), 2);
        assert_eq!(bar.index(0, 3), (PAGES - 1) * SLOTS + 3);
        assert_eq!(bar.index(1, 3), 3, "the second row wraps to the first page");

        bar.turn(true);
        assert_eq!(bar.page, 0);
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
