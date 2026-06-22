use crate::converters::read_system_en;
use crate::decompile::decompile;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::JobData;
use std::path::Path;

const MAX_VALID_JOB_ID: i64 = 999_998;

/// Sprite resource names (JobNameTable) and the `JOBID`/`JTtbl` symbol map that
/// pcjobname's keys resolve against still come from the GRF; only the English
/// PC display names (PCJobNameTable) are sourced from SystemEN below.
const GRF_LUB_PATHS: [&str; 3] = [
    "data/luafiles514/lua files/datainfo/jobidentity.lub",
    "data/luafiles514/lua files/datainfo/npcidentity.lub",
    "data/luafiles514/lua files/datainfo/jobname.lub",
];

const PCJOBNAME_PATH: &str = "LuaFiles514/pcjobname.lub";

pub fn run(vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    let lua = lua::new_vm().map_err(lua_err)?;

    lua::exec_chunk(&lua, &read_grf_lub(vfs, GRF_LUB_PATHS[0])?).map_err(lua_err)?;
    lua::install_job_metatables(&lua).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, GRF_LUB_PATHS[1])?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, GRF_LUB_PATHS[2])?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_system_en(PCJOBNAME_PATH)?).map_err(lua_err)?;

    let job_data = extract_job_data(&lua)?;

    let ron = ron::ser::to_string_pretty(&job_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("job_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    println!(
        "job_data.ron: {} npc sprites, {} display names",
        job_data.npc_sprites.len(),
        job_data.display_names.len()
    );
    Ok(())
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_grf_lub(vfs: &GrfVfs, path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(path)
        .with_context(|| format!("job lub not found in GRFs: {path}"))?;
    decompile(&bytes).with_context(|| format!("decompiling {path}"))
}

fn extract_job_data(lua: &mlua::Lua) -> anyhow::Result<JobData> {
    let globals = lua.globals();
    let mut job_data = JobData::default();

    let job_name_table = globals
        .get::<mlua::Table>("JobNameTable")
        .map_err(lua_err)?;
    for pair in job_name_table.pairs::<mlua::Value, mlua::String>() {
        let (key, name) = pair.map_err(lua_err)?;
        if let Some(id) = job_id(&key) {
            job_data
                .npc_sprites
                .insert(id, decode_euckr(name.as_bytes().as_ref()));
        }
    }

    let pc_job_name_table = globals
        .get::<mlua::Table>("PCJobNameTable")
        .map_err(lua_err)?;
    for pair in pc_job_name_table.pairs::<mlua::Value, mlua::String>() {
        let (key, name) = pair.map_err(lua_err)?;
        if let Some(id) = job_id(&key).filter(|&id| i64::from(id) <= MAX_VALID_JOB_ID) {
            job_data
                .display_names
                .insert(id, decode_euckr(name.as_bytes().as_ref()));
        }
    }

    Ok(job_data)
}

fn job_id(key: &mlua::Value) -> Option<u32> {
    match key {
        mlua::Value::Integer(id) => Some(*id as u32),
        mlua::Value::Number(id) => Some(*id as u32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_decoded_names_and_filters_out_of_range() -> anyhow::Result<()> {
        let lua = lua::new_vm().map_err(lua_err)?;
        lua::exec_chunk(&lua, b"JTtbl = { NOVICE = 0 }").map_err(lua_err)?;
        lua::install_job_metatables(&lua).map_err(lua_err)?;

        let (korean, _, _) = encoding_rs::EUC_KR.encode("초보자");
        let mut chunk = b"JobNameTable = { [0] = \"".to_vec();
        chunk.extend_from_slice(&korean);
        chunk.extend_from_slice(
            b"\" }\nPCJobNameTable = { [0] = \"Novice\", [999999] = \"OutOfRange\" }",
        );
        lua::exec_chunk(&lua, &chunk).map_err(lua_err)?;

        let job_data = extract_job_data(&lua)?;

        assert_eq!(
            job_data.npc_sprites.get(&0).map(String::as_str),
            Some("초보자")
        );
        assert_eq!(
            job_data.display_names.get(&0).map(String::as_str),
            Some("Novice")
        );
        assert!(!job_data.display_names.contains_key(&999_999));

        Ok(())
    }
}
