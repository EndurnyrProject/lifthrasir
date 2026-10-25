use crate::OperationReport;
use crate::decompile::decompile;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::RandomOptionData;
use std::path::Path;

/// `enumvar.lub` defines `EnumVAR = { VAR_* = { <option id>, <something> } }`;
/// `addrandomoptionnametable.lub` defines
/// `NameTable_VAR = { [EnumVAR.VAR_*[1]] = "<template>" }`, and
/// `addrandomoption_f.lub` defines the helpers the client uses to read it.
/// Execing all three in one VM resolves the enum keys to numeric option ids.
const ENUMVAR_PATH: &str = "data/luafiles514/lua files/datainfo/enumvar.lub";
const NAMETABLE_PATH: &str = "data/luafiles514/lua files/datainfo/addrandomoptionnametable.lub";
const HELPERS_PATH: &str = "data/luafiles514/lua files/datainfo/addrandomoption_f.lub";

pub fn run(vfs: &GrfVfs, out: &Path) -> anyhow::Result<OperationReport> {
    let lua = lua::new_vm_unbounded().map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, ENUMVAR_PATH)?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, HELPERS_PATH)?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, NAMETABLE_PATH)?).map_err(lua_err)?;

    let option_data = extract_random_option_data(&lua)?;

    let ron = ron::ser::to_string_pretty(&option_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("random_option_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    let mut report = OperationReport {
        outputs: vec![dest],
        ..Default::default()
    };
    report.info(format!(
        "random_option_data.ron: {} options",
        option_data.names.len()
    ));
    Ok(report)
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_grf_lub(vfs: &GrfVfs, path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(path)
        .with_context(|| format!("random option lub not found in GRFs: {path}"))?;
    if bytes.starts_with(b"\x1bLua") {
        return decompile(&bytes).with_context(|| format!("decompiling {path}"));
    }
    Ok(bytes)
}

fn extract_random_option_data(lua: &mlua::Lua) -> anyhow::Result<RandomOptionData> {
    let table = lua
        .globals()
        .get::<mlua::Table>("NameTable_VAR")
        .map_err(lua_err)?;

    let mut option_data = RandomOptionData::default();
    for pair in table.pairs::<mlua::Value, mlua::LuaString>() {
        let (key, template) = pair.map_err(lua_err)?;
        if let Some(option_id) = option_id(&key) {
            option_data
                .names
                .insert(option_id, decode_euckr(template.as_bytes().as_ref()));
        }
    }

    Ok(option_data)
}

fn option_id(key: &mlua::Value) -> Option<u32> {
    match key {
        mlua::Value::Integer(id) => u32::try_from(*id).ok(),
        mlua::Value::Number(id) => u32::try_from(*id as i64).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &[u8]) -> RandomOptionData {
        let lua = lua::new_vm_unbounded().unwrap();
        lua::exec_chunk(&lua, src).unwrap();
        extract_random_option_data(&lua).unwrap()
    }

    #[test]
    fn resolves_enum_keys_to_option_ids() {
        let src = br#"
            EnumVAR = {
                VAR_STRAMOUNT = { 3, 103 },
                VAR_ATKPERCENT = { 13, 113 },
                EnumVAR_LAST = { "END" }
            }
            NameTable_VAR = {
                [EnumVAR.VAR_STRAMOUNT[1]] = "STR + %d",
                [EnumVAR.VAR_ATKPERCENT[1]] = "ATK + %d%%",
                [EnumVAR.EnumVAR_LAST[1]] = "END"
            }
        "#;

        let data = extract(src);
        assert_eq!(data.names.get(&3).map(String::as_str), Some("STR + %d"));
        assert_eq!(data.names.get(&13).map(String::as_str), Some("ATK + %d%%"));
        assert_eq!(data.names.len(), 2);
    }

    #[test]
    fn decodes_euckr_templates() {
        let (korean, _, _) = encoding_rs::EUC_KR.encode("공격력 + %d");
        let mut src = b"NameTable_VAR = { [1] = \"".to_vec();
        src.extend_from_slice(&korean);
        src.extend_from_slice(b"\" }");

        let data = extract(&src);
        assert_eq!(data.names.get(&1).map(String::as_str), Some("공격력 + %d"));
    }
}
