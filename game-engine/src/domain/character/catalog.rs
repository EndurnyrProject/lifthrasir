use crate::domain::entities::character::components::Gender;
use bevy::prelude::*;
use std::collections::BTreeMap;

/// Single head style entry with all variants
#[derive(Debug, Clone)]
pub struct HeadStyleEntry {
    pub id: u16,
    pub gender: Gender,
    pub sprite_path: String,
    pub act_path: String,
    pub available_colors: Vec<u16>, // Discovered palette color IDs
}

/// Hair catalog organized by gender and style ID
#[derive(Resource, Default)]
pub struct HeadStyleCatalog {
    male: BTreeMap<u16, HeadStyleEntry>,   // Sorted by ID
    female: BTreeMap<u16, HeadStyleEntry>, // Sorted by ID
}

impl HeadStyleCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get head style by gender and ID
    pub fn get(&self, gender: Gender, id: u16) -> Option<&HeadStyleEntry> {
        match gender {
            Gender::Male => self.male.get(&id),
            Gender::Female => self.female.get(&id),
        }
    }

    /// Get all head styles for a gender (sorted by ID)
    pub fn get_all(&self, gender: Gender) -> Vec<&HeadStyleEntry> {
        match gender {
            Gender::Male => self.male.values().collect(),
            Gender::Female => self.female.values().collect(),
        }
    }

    /// Get all head style IDs for a gender (sorted)
    pub fn get_all_ids(&self, gender: Gender) -> Vec<u16> {
        match gender {
            Gender::Male => self.male.keys().copied().collect(),
            Gender::Female => self.female.keys().copied().collect(),
        }
    }

    /// Get available hair colors for a specific style
    pub fn get_colors(&self, gender: Gender, id: u16) -> Option<&Vec<u16>> {
        self.get(gender, id).map(|entry| &entry.available_colors)
    }

    /// Total number of styles (both genders)
    pub fn total_count(&self) -> usize {
        self.male.len() + self.female.len()
    }

    /// Count of male styles
    pub fn male_count(&self) -> usize {
        self.male.len()
    }

    /// Count of female styles
    pub fn female_count(&self) -> usize {
        self.female.len()
    }

    /// Add a head style entry
    pub(crate) fn add(&mut self, entry: HeadStyleEntry) {
        match entry.gender {
            Gender::Male => {
                self.male.insert(entry.id, entry);
            }
            Gender::Female => {
                self.female.insert(entry.id, entry);
            }
        }
    }
}
