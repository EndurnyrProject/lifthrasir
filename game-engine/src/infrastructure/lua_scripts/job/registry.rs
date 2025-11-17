use crate::domain::entities::character::components::core::Gender;
use bevy::prelude::*;
use mlua::prelude::*;
use mlua::Error as LuaError;
use std::collections::HashMap;

use super::player_jobs::{get_player_job_sprite_mapping, is_player_job};

const UNKNOWN_JOB_SENTINEL: i32 = -999_999;
const MAX_VALID_JOB_ID: i64 = 999_998;

#[derive(Resource)]
pub struct JobSpriteRegistry {
    player_jobs: HashMap<u32, &'static str>,
    npc_sprites: HashMap<u32, String>,
    display_names: HashMap<u32, String>,
}

impl JobSpriteRegistry {
    pub fn from_lua_sources(
        job_identity_src: &str,
        npc_identity_src: &str,
        job_name_src: &str,
        pc_job_name_src: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let lua = Lua::new();

        lua.set_hook(
            mlua::HookTriggers::new().every_nth_instruction(200_000),
            |_lua, _debug| {
                Err(LuaError::RuntimeError(
                    "Lua script execution budget exceeded (200k instructions)".into(),
                ))
            },
        )?;

        lua.load(job_identity_src).exec()?;

        lua.load("JOBID = JTtbl").exec()?;
        lua.load("pcJobTbl = JTtbl").exec()?;

        lua.load(format!(
            r#"
            setmetatable(JOBID, {{
                __index = function(t, k)
                    return {}
                end
            }})
        "#,
            UNKNOWN_JOB_SENTINEL
        ))
        .exec()?;

        lua.load(npc_identity_src).exec()?;
        lua.load(job_name_src).exec()?;
        lua.load(pc_job_name_src).set_name("pcjobname").exec()?;

        let player_jobs = get_player_job_sprite_mapping();
        let npc_sprites = Self::extract_job_name_table(&lua)?;
        let display_names = Self::extract_pc_job_name_table(&lua)?;

        info!("Loaded {} player job mappings", player_jobs.len());
        info!("Loaded {} NPC sprite mappings", npc_sprites.len());
        info!("Loaded {} display names", display_names.len());

        Ok(Self {
            player_jobs,
            npc_sprites,
            display_names,
        })
    }

    fn extract_job_name_table(lua: &Lua) -> LuaResult<HashMap<u32, String>> {
        let globals = lua.globals();
        let job_name_table: LuaTable = globals.get("JobNameTable")?;

        let mut sprites = HashMap::new();

        for pair in job_name_table.pairs::<LuaValue, String>() {
            let (key, name) = pair?;

            match key {
                LuaValue::Integer(id) => {
                    sprites.insert(id as u32, name);
                }
                LuaValue::Number(id) => {
                    sprites.insert(id as u32, name);
                }
                _ => {}
            }
        }

        Ok(sprites)
    }

    fn extract_pc_job_name_table(lua: &Lua) -> LuaResult<HashMap<u32, String>> {
        let globals = lua.globals();
        let pc_job_name_table: LuaTable = globals.get("PCJobNameTable")?;

        let mut names = HashMap::new();
        for pair in pc_job_name_table.pairs::<LuaValue, String>() {
            let (key, name) = pair?;
            match key {
                LuaValue::Integer(id) => {
                    if (0..=MAX_VALID_JOB_ID).contains(&id) {
                        names.insert(id as u32, name);
                    }
                }
                LuaValue::Number(id) => {
                    if (0.0..=(MAX_VALID_JOB_ID as f64)).contains(&id) {
                        names.insert(id as u32, name);
                    }
                }
                _ => {}
            }
        }

        Ok(names)
    }

    pub fn get_sprite_name(&self, jt_id: u32) -> Option<&str> {
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

    pub fn get_display_name_gendered(&self, jt_id: u32, _gender: Gender) -> Option<&str> {
        self.get_display_name(jt_id)
    }
}
