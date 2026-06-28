use crate::domain::assets::patterns;
use crate::domain::entities::character::components::core::Gender;
use bevy::prelude::*;
use std::collections::HashMap;

use super::player_jobs::{get_player_job_sprite_mapping, is_player_job};

/// JT_WARPNPC. The official jobname.lub maps this to a placeholder ("1_ETC_01");
/// the real client special-cases it to the animated portal sprite instead.
pub const WARP_JOB_ID: u32 = 45;
const WARP_SPRITE_NAME: &str = "portal";

#[derive(Resource)]
pub struct JobSpriteRegistry {
    player_jobs: HashMap<u32, &'static str>,
    npc_sprites: HashMap<u32, String>,
    display_names: HashMap<u32, String>,
}

impl JobSpriteRegistry {
    pub fn from_job_data(data: lifthrasir_data::JobData) -> Self {
        Self {
            player_jobs: get_player_job_sprite_mapping(),
            npc_sprites: data.npc_sprites.into_iter().collect(),
            display_names: data.display_names.into_iter().collect(),
        }
    }

    pub fn get_sprite_name(&self, jt_id: u32) -> Option<&str> {
        if jt_id == WARP_JOB_ID {
            return Some(WARP_SPRITE_NAME);
        }

        if is_player_job(jt_id) {
            if let Some(sprite) = self.player_jobs.get(&jt_id) {
                return Some(sprite);
            }
        }

        if let Some(sprite) = self.npc_sprites.get(&jt_id) {
            return Some(sprite.as_str());
        }

        warn!("Unknown job ID: {}, using fallback", jt_id);
        None
    }

    pub fn get_display_name(&self, jt_id: u32) -> Option<&str> {
        let display_name = self.display_names.get(&jt_id).map(|s| s.as_str());
        if display_name.is_none() {
            warn!("Unknown display name for job ID: {}", jt_id);
        }
        display_name
    }

    /// Display name lookup without the missing-id `warn!`, for callers that have
    /// their own fallback and run often enough that a warning would spam the log.
    pub fn try_display_name(&self, jt_id: u32) -> Option<&str> {
        self.display_names.get(&jt_id).map(|s| s.as_str())
    }

    pub fn get_body_sprite_path(&self, jt_id: u32, gender: u8) -> Option<String> {
        let sprite_name = self.get_sprite_name(jt_id)?;
        let gender_enum = Gender::from(gender);
        Some(patterns::body_sprite_path(gender_enum, sprite_name))
    }

    pub fn get_hair_sprite_path(&self, hair_id: u16, gender: u8) -> String {
        let gender_enum = Gender::from(gender);
        patterns::hair_sprite_path(gender_enum, hair_id)
    }

    pub fn get_hair_palette_path(&self, hair_id: u16, gender: u8, color: u16) -> Option<String> {
        if color == 0 {
            return None;
        }
        let gender_enum = Gender::from(gender);
        Some(patterns::hair_palette_path(hair_id, gender_enum, color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::JobData;

    fn fixture() -> JobData {
        let mut data = JobData::default();
        data.npc_sprites.insert(46, "1_ETC_01".to_string());
        data.display_names.insert(0, "Novice".to_string());
        data
    }

    #[test]
    fn from_job_data_wires_npc_player_and_display_lookups() {
        let registry = JobSpriteRegistry::from_job_data(fixture());

        assert_eq!(registry.get_sprite_name(46), Some("1_ETC_01"));
        assert!(is_player_job(1));
        assert_eq!(registry.get_sprite_name(1), Some("검사"));
        assert_eq!(registry.get_display_name(0), Some("Novice"));
    }

    #[test]
    fn warp_job_overrides_placeholder_with_portal_sprite() {
        let mut data = JobData::default();
        data.npc_sprites.insert(WARP_JOB_ID, "1_ETC_01".to_string());
        let registry = JobSpriteRegistry::from_job_data(data);

        assert_eq!(registry.get_sprite_name(WARP_JOB_ID), Some("portal"));
    }
}
