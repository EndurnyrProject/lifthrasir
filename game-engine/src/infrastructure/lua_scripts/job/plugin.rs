use super::registry::JobSpriteRegistry;
use crate::infrastructure::lua_scripts::loader::{LuaBytecode, LuaBytecodeLoader};
use bevy::prelude::*;

#[derive(Resource)]
struct LuaFileHandles {
    job_identity: Handle<LuaBytecode>,
    npc_identity: Handle<LuaBytecode>,
    job_name: Handle<LuaBytecode>,
    pc_job_name: Handle<LuaBytecode>,
}

pub struct JobSystemPlugin;

impl Plugin for JobSystemPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LuaBytecode>()
            .init_asset_loader::<LuaBytecodeLoader>()
            .add_systems(Startup, start_loading_lua_files)
            .add_systems(Update, process_loaded_lua_files);
    }
}

fn start_loading_lua_files(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handles = LuaFileHandles {
        job_identity: asset_server.load("ro://data/luafiles514/lua files/datainfo/jobidentity.lub"),
        npc_identity: asset_server.load("ro://data/luafiles514/lua files/datainfo/npcidentity.lub"),
        job_name: asset_server.load("ro://data/luafiles514/lua files/datainfo/jobname.lub"),
        pc_job_name: asset_server.load("ro://data/luafiles514/lua files/datainfo/pcjobname.lub"),
    };

    commands.insert_resource(handles);
    info!("Loading Lua job system files");
}

fn process_loaded_lua_files(
    mut commands: Commands,
    handles: Option<Res<LuaFileHandles>>,
    lua_assets: Res<Assets<LuaBytecode>>,
    mut attempts: Local<u8>,
) {
    let Some(handles) = handles else { return };

    let Some(job_identity) = lua_assets.get(&handles.job_identity) else {
        return;
    };
    let Some(npc_identity) = lua_assets.get(&handles.npc_identity) else {
        return;
    };
    let Some(job_name) = lua_assets.get(&handles.job_name) else {
        return;
    };
    let Some(pc_job_name) = lua_assets.get(&handles.pc_job_name) else {
        return;
    };

    let job_identity_src = &job_identity.source;
    let npc_identity_src = &npc_identity.source;
    let job_name_src = &job_name.source;
    let pc_job_name_src = &pc_job_name.source;

    match JobSpriteRegistry::from_lua_sources(
        job_identity_src,
        npc_identity_src,
        job_name_src,
        pc_job_name_src,
    ) {
        Ok(registry) => {
            commands.insert_resource(registry);
            commands.remove_resource::<LuaFileHandles>();
            info!("Lua Job Sprite registry created successfully");
        }
        Err(e) => {
            *attempts = attempts.saturating_add(1);
            if *attempts == 1 {
                error!("Failed to create job sprite registry: {}", e);
            } else if *attempts >= 5 {
                error!(
                    "Stopping attempts to create JobSpriteRegistry after {} failures",
                    *attempts
                );
                commands.remove_resource::<LuaFileHandles>();
            }
        }
    }
}
