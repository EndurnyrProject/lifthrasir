use crate::decompile::decompile;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::AccessoryData;
use std::path::Path;

/// `accessoryid.lub` defines `ACCESSORY_IDs = { ACCESSORY_* = <view id> }`;
/// `accname.lub` defines `AccNameTable = { [ACCESSORY_IDs.ACCESSORY_*] = "<sprite name>" }`.
/// Execing both in one VM resolves the enum keys to numeric view ids, so the
/// resulting `AccNameTable` is keyed directly by view id. Both ship as bytecode.
const ACCESSORYID_PATH: &str = "data/luafiles514/lua files/datainfo/accessoryid.lub";
const ACCNAME_PATH: &str = "data/luafiles514/lua files/datainfo/accname.lub";

pub fn run(vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    let lua = lua::new_vm_unbounded().map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, ACCESSORYID_PATH)?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, ACCNAME_PATH)?).map_err(lua_err)?;

    let accessory_data = extract_accessory_data(&lua)?;

    let ron = ron::ser::to_string_pretty(&accessory_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("accessory_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    println!(
        "accessory_data.ron: {} accessories",
        accessory_data.names.len()
    );
    Ok(())
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_grf_lub(vfs: &GrfVfs, path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(path)
        .with_context(|| format!("accessory lub not found in GRFs: {path}"))?;
    if bytes.starts_with(b"\x1bLua") {
        return decompile(&bytes).with_context(|| format!("decompiling {path}"));
    }
    Ok(bytes)
}

fn extract_accessory_data(lua: &mlua::Lua) -> anyhow::Result<AccessoryData> {
    let table = lua
        .globals()
        .get::<mlua::Table>("AccNameTable")
        .map_err(lua_err)?;

    let mut accessory_data = AccessoryData::default();
    for pair in table.pairs::<mlua::Value, mlua::String>() {
        let (key, name) = pair.map_err(lua_err)?;
        if let Some(view_id) = view_id(&key) {
            accessory_data
                .names
                .insert(view_id, decode_euckr(name.as_bytes().as_ref()));
        }
    }

    Ok(accessory_data)
}

fn view_id(key: &mlua::Value) -> Option<u16> {
    match key {
        mlua::Value::Integer(id) => u16::try_from(*id).ok(),
        mlua::Value::Number(id) => u16::try_from(*id as i64).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &[u8]) -> AccessoryData {
        let lua = lua::new_vm_unbounded().unwrap();
        lua::exec_chunk(&lua, src).unwrap();
        extract_accessory_data(&lua).unwrap()
    }

    #[test]
    fn joins_id_and_name_tables_by_view_id() {
        let src = br#"
            ACCESSORY_IDs = { ACCESSORY_GOGGLES = 1, ACCESSORY_GLASS = 3 }
            AccNameTable = {
                [ACCESSORY_IDs.ACCESSORY_GOGGLES] = "_GOGGLE",
                [ACCESSORY_IDs.ACCESSORY_GLASS] = "_GLASS"
            }
        "#;

        let data = extract(src);
        assert_eq!(data.names.get(&1).map(String::as_str), Some("_GOGGLE"));
        assert_eq!(data.names.get(&3).map(String::as_str), Some("_GLASS"));
        assert_eq!(data.names.len(), 2);
    }

    #[test]
    fn preserves_leading_separator_and_decodes_euckr() {
        let (korean, _, _) = encoding_rs::EUC_KR.encode("고글");
        let mut src = b"AccNameTable = { [1] = \"_".to_vec();
        src.extend_from_slice(&korean);
        src.extend_from_slice(b"\" }");

        let data = extract(&src);
        assert_eq!(data.names.get(&1).map(String::as_str), Some("_고글"));
    }
}
