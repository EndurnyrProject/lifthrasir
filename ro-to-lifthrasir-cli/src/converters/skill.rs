use crate::decompile::decompile;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::{SkillData, SkillMeta};
use std::path::Path;

/// `skillid.lub` is compiled Lua bytecode in `data.grf`; `skillinfolist.lub` and
/// `skilldescript.lub` ship as plaintext from `en.grf` (English translation).
/// `read_grf_lub` decompiles only the bytecode ones.
const SKILLID_PATH: &str = "data/luafiles514/lua files/skillinfoz/skillid.lub";
const SKILLINFOLIST_PATH: &str = "data/luafiles514/lua files/skillinfoz/skillinfolist.lub";
const SKILLDESCRIPT_PATH: &str = "data/luafiles514/lua files/skillinfoz/skilldescript.lub";

/// `JOBID` is defined in jobidentity.lub (the job converter's domain). Skill
/// parsing only needs it as table keys in fields we discard, so a metatable
/// returning a sentinel for any key avoids pulling in job data.
const STUB_JOBID: &[u8] =
    b"JOBID = setmetatable({}, { __index = function(_, k) return tostring(k) end })";

pub fn run(vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    // SKILL_INFO_LIST / SKILL_DESCRIPT key off `[SKID.NAME]`, so SKID must exist first.
    // The tables are large; use the unbounded VM like the item converter.
    let lua = lua::new_vm_unbounded().map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, SKILLID_PATH)?).map_err(lua_err)?;
    // skillinfolist's out-of-scope NeedSkillList subtables key off `JOBID.JT_*`;
    // we never read them, so stub JOBID to resolve any key rather than load job data.
    lua::exec_chunk(&lua, STUB_JOBID).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, SKILLINFOLIST_PATH)?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, SKILLDESCRIPT_PATH)?).map_err(lua_err)?;

    let skill_data = extract_skill_data(&lua)?;

    let ron = ron::ser::to_string_pretty(&skill_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("skill_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    println!("skill_data.ron: {} skills", skill_data.skills.len());
    Ok(())
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_grf_lub(vfs: &GrfVfs, path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(path)
        .with_context(|| format!("skill lub not found in GRFs: {path}"))?;
    if bytes.starts_with(b"\x1bLua") {
        return decompile(&bytes).with_context(|| format!("decompiling {path}"));
    }
    Ok(bytes)
}

fn extract_skill_data(lua: &mlua::Lua) -> anyhow::Result<SkillData> {
    let globals = lua.globals();
    let info_list = globals
        .get::<mlua::Table>("SKILL_INFO_LIST")
        .map_err(lua_err)?;
    let descript = globals
        .get::<mlua::Table>("SKILL_DESCRIPT")
        .map_err(lua_err)?;

    let mut skill_data = SkillData::default();
    for pair in info_list.pairs::<mlua::Value, mlua::Table>() {
        let (key, sub) = pair.map_err(lua_err)?;
        if let Some(id) = skill_id(&key) {
            let description = descript
                .get::<Option<mlua::Table>>(id)
                .ok()
                .flatten()
                .map(|t| euckr_lines(&t))
                .unwrap_or_default();
            skill_data.skills.insert(id, parse_skill(&sub, description));
        }
    }

    Ok(skill_data)
}

fn skill_id(key: &mlua::Value) -> Option<u32> {
    match key {
        mlua::Value::Integer(id) => Some(*id as u32),
        mlua::Value::Number(id) => Some(*id as u32),
        _ => None,
    }
}

fn parse_skill(sub: &mlua::Table, description: Vec<String>) -> SkillMeta {
    SkillMeta {
        name: euckr_field_index(sub, 1),
        display_name: euckr_field(sub, "SkillName"),
        description,
        max_level: sub.get::<Option<u8>>("MaxLv").ok().flatten().unwrap_or(0),
        sp_cost: number_seq(sub, "SpAmount"),
        attack_range: number_seq(sub, "AttackRange"),
    }
}

fn euckr_field(sub: &mlua::Table, key: &str) -> String {
    match sub.get::<Option<mlua::String>>(key) {
        Ok(Some(s)) => decode_euckr(s.as_bytes().as_ref()),
        _ => String::default(),
    }
}

fn euckr_field_index(sub: &mlua::Table, idx: i64) -> String {
    match sub.get::<Option<mlua::String>>(idx) {
        Ok(Some(s)) => decode_euckr(s.as_bytes().as_ref()),
        _ => String::default(),
    }
}

fn euckr_lines(table: &mlua::Table) -> Vec<String> {
    table
        .sequence_values::<mlua::String>()
        .filter_map(Result::ok)
        .map(|s| decode_euckr(s.as_bytes().as_ref()))
        .collect()
}

fn number_seq<T: mlua::FromLua>(sub: &mlua::Table, key: &str) -> Vec<T> {
    let Ok(Some(table)) = sub.get::<Option<mlua::Table>>(key) else {
        return Vec::new();
    };
    table
        .sequence_values::<T>()
        .filter_map(Result::ok)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &[u8]) -> SkillData {
        let lua = lua::new_vm_unbounded().unwrap();
        lua::exec_chunk(&lua, src).unwrap();
        extract_skill_data(&lua).unwrap()
    }

    #[test]
    fn parses_active_skill_with_sp_range_and_description() {
        let src = br#"
            SKID = { SM_BASH = 5 }
            SKILL_INFO_LIST = {
                [SKID.SM_BASH] = {
                    "SM_BASH",
                    SkillName = "Bash",
                    MaxLv = 10,
                    SpAmount = { 8, 8, 8, 8, 8, 15, 15, 15, 15, 15 },
                    AttackRange = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1 }
                }
            }
            SKILL_DESCRIPT = {
                [SKID.SM_BASH] = { "Bash", "Strike an enemy." }
            }
        "#;

        let data = extract(src);
        let bash = data.skills.get(&5).expect("SM_BASH present");
        assert_eq!(bash.name, "SM_BASH");
        assert_eq!(bash.display_name, "Bash");
        assert_eq!(bash.max_level, 10);
        assert_eq!(bash.sp_cost, vec![8, 8, 8, 8, 8, 15, 15, 15, 15, 15]);
        assert_eq!(bash.attack_range, vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
        assert_eq!(bash.description, vec!["Bash", "Strike an enemy."]);
    }

    #[test]
    fn passive_skill_defaults_empty_sp_range_and_description() {
        let src = br#"
            SKID = { SM_ENDURE = 8 }
            SKILL_INFO_LIST = {
                [SKID.SM_ENDURE] = {
                    "SM_ENDURE",
                    SkillName = "Endure",
                    MaxLv = 10
                }
            }
            SKILL_DESCRIPT = {}
        "#;

        let data = extract(src);
        let endure = data.skills.get(&8).expect("SM_ENDURE present");
        assert_eq!(endure.name, "SM_ENDURE");
        assert_eq!(endure.display_name, "Endure");
        assert_eq!(endure.max_level, 10);
        assert!(endure.sp_cost.is_empty());
        assert!(endure.attack_range.is_empty());
        assert!(endure.description.is_empty());
    }

    #[test]
    fn decodes_euckr_display_name() {
        let (korean, _, _) = encoding_rs::EUC_KR.encode("초보자");
        let mut src =
            b"SKID = { TEST = 1 }\nSKILL_INFO_LIST = { [SKID.TEST] = { \"TEST\", SkillName = \""
                .to_vec();
        src.extend_from_slice(&korean);
        src.extend_from_slice(b"\" } }\nSKILL_DESCRIPT = {}");

        let data = extract(&src);
        assert_eq!(
            data.skills.get(&1).map(|s| s.display_name.as_str()),
            Some("초보자")
        );
    }
}
