use crate::decompile::decompile;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::WeaponData;
use std::path::Path;

/// `weapontable.lub` defines `Weapon_IDs` (the `WEAPONTYPE_*` view-id enum) and
/// keys `WeaponNameTable` (view id -> sprite suffix) / `WeaponHitWaveNameTable`
/// (view id -> hit wav) by those ids, plus `BowTypeList` (array of bow view ids).
/// Ships as bytecode.
const WEAPONTABLE_PATH: &str = "data/luafiles514/lua files/datainfo/weapontable.lub";

pub fn run(vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    let lua = lua::new_vm_unbounded().map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, WEAPONTABLE_PATH)?).map_err(lua_err)?;

    let weapon_data = extract_weapon_data(&lua)?;

    let ron = ron::ser::to_string_pretty(&weapon_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("weapon_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    println!(
        "weapon_data.ron: {} names, {} hit_sounds, {} bow_types",
        weapon_data.names.len(),
        weapon_data.hit_sounds.len(),
        weapon_data.bow_types.len()
    );
    Ok(())
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_grf_lub(vfs: &GrfVfs, path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(path)
        .with_context(|| format!("weapon lub not found in GRFs: {path}"))?;
    if bytes.starts_with(b"\x1bLua") {
        return decompile(&bytes).with_context(|| format!("decompiling {path}"));
    }
    Ok(bytes)
}

fn extract_weapon_data(lua: &mlua::Lua) -> anyhow::Result<WeaponData> {
    let mut weapon_data = WeaponData::default();

    let names = lua
        .globals()
        .get::<mlua::Table>("WeaponNameTable")
        .map_err(lua_err)?;
    for pair in names.pairs::<mlua::Value, mlua::String>() {
        let (key, name) = pair.map_err(lua_err)?;
        if let Some(view_id) = view_id(&key) {
            weapon_data
                .names
                .insert(view_id, decode_euckr(name.as_bytes().as_ref()));
        }
    }

    let hit_sounds = lua
        .globals()
        .get::<mlua::Table>("WeaponHitWaveNameTable")
        .map_err(lua_err)?;
    for pair in hit_sounds.pairs::<mlua::Value, mlua::String>() {
        let (key, name) = pair.map_err(lua_err)?;
        if let Some(view_id) = view_id(&key) {
            weapon_data
                .hit_sounds
                .insert(view_id, decode_euckr(name.as_bytes().as_ref()));
        }
    }

    let bow_types = lua
        .globals()
        .get::<mlua::Table>("BowTypeList")
        .map_err(lua_err)?;
    for value in bow_types.sequence_values::<mlua::Value>() {
        if let Some(view_id) = view_id(&value.map_err(lua_err)?) {
            weapon_data.bow_types.insert(view_id);
        }
    }

    Ok(weapon_data)
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

    fn extract(src: &[u8]) -> WeaponData {
        let lua = lua::new_vm_unbounded().unwrap();
        lua::exec_chunk(&lua, src).unwrap();
        extract_weapon_data(&lua).unwrap()
    }

    #[test]
    fn extracts_names_hit_sounds_and_bow_types() {
        let (sword, _, _) = encoding_rs::EUC_KR.encode("검");
        let mut src = b"Weapon_IDs = { WEAPONTYPE_SWORD = 2, WEAPONTYPE_BOW = 11 }\n".to_vec();
        src.extend_from_slice(b"WeaponNameTable = { [Weapon_IDs.WEAPONTYPE_SWORD] = \"_");
        src.extend_from_slice(&sword);
        src.extend_from_slice(b"\" }\n");
        src.extend_from_slice(
            b"WeaponHitWaveNameTable = { [Weapon_IDs.WEAPONTYPE_SWORD] = \"_hit_sword.wav\" }\n",
        );
        src.extend_from_slice(b"BowTypeList = { Weapon_IDs.WEAPONTYPE_BOW }\n");

        let data = extract(&src);
        assert_eq!(data.names.get(&2).map(String::as_str), Some("_검"));
        assert_eq!(
            data.hit_sounds.get(&2).map(String::as_str),
            Some("_hit_sword.wav")
        );
        assert!(data.bow_types.contains(&11));
    }

    #[test]
    fn skips_out_of_range_and_non_integer_keys() {
        let src = br#"
            WeaponNameTable = { [2] = "_sword", ["bogus"] = "_x", [99999] = "_big" }
            WeaponHitWaveNameTable = {}
            BowTypeList = {}
        "#;

        let data = extract(src);
        assert_eq!(data.names.len(), 1);
        assert_eq!(data.names.get(&2).map(String::as_str), Some("_sword"));
    }
}
