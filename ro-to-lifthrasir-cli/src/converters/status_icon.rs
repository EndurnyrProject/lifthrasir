use crate::decompile::decompile;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::{StatusIconData, StatusIconEntry};
use std::path::Path;

const EFSTIDS_PATH: &str = "data/luafiles514/lua files/stateicon/efstids.lub";
const IMGINFO_PATH: &str = "data/luafiles514/lua files/stateicon/stateiconimginfo.lub";
const INFO_PATH: &str = "data/luafiles514/lua files/stateicon/stateiconinfo.lub";

/// EFST names referenced by the img/info tables that are absent from `EFST_IDs`
/// resolve to this sentinel via a metatable, so a missing name never crashes the
/// `[EFST_IDs.MISSING] = ...` assignment. Sentinel entries are dropped by `efst_id`.
const UNKNOWN_EFST_SENTINEL: i64 = -999_999;

const LUA_BYTECODE_MAGIC: &[u8] = b"\x1bLua";

pub fn run(vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    let lua = lua::new_vm_unbounded().map_err(lua_err)?;

    lua::exec_chunk(&lua, &read_grf_lub(vfs, EFSTIDS_PATH)?).map_err(lua_err)?;
    install_efst_fallback(&lua)?;
    lua::exec_chunk(&lua, &read_grf_lub(vfs, IMGINFO_PATH)?).map_err(lua_err)?;
    lua::exec_chunk(&lua, &read_state_icon_info(vfs)?).map_err(lua_err)?;

    let data = extract_status_icons(&lua)?;

    let ron = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("status_icons.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    println!("status_icons.ron: {} icons", data.icons.len());
    Ok(())
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_grf_lub(vfs: &GrfVfs, path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(path)
        .with_context(|| format!("status icon lub not found in GRFs: {path}"))?;
    decompile(&bytes).with_context(|| format!("decompiling {path}"))
}

/// The English display names live in the en.grf plaintext copy of
/// `stateiconinfo.lub` (the data.grf copy is Korean bytecode). The GRF layer
/// serves en.grf first, so `vfs.read` returns the plaintext copy; guard against a
/// GRF ordering that yields bytecode by decompiling only when it is bytecode.
fn read_state_icon_info(vfs: &GrfVfs) -> anyhow::Result<Vec<u8>> {
    let bytes = vfs
        .read(INFO_PATH)
        .with_context(|| format!("status icon info not found in GRFs: {INFO_PATH}"))?;
    if bytes.starts_with(LUA_BYTECODE_MAGIC) {
        return decompile(&bytes).with_context(|| format!("decompiling {INFO_PATH}"));
    }
    Ok(bytes)
}

fn install_efst_fallback(lua: &mlua::Lua) -> anyhow::Result<()> {
    lua.load(format!(
        r#"
        setmetatable(EFST_IDs, {{
            __index = function(t, k)
                return {UNKNOWN_EFST_SENTINEL}
            end
        }})
        "#
    ))
    .exec()
    .map_err(lua_err)
}

fn extract_status_icons(lua: &mlua::Lua) -> anyhow::Result<StatusIconData> {
    let mut images = collect_images(lua)?;
    let names = collect_names(lua)?;

    let mut data = StatusIconData::default();
    for (id, name) in names {
        let image = images.remove(&id).unwrap_or_default();
        if name.is_empty() && image.is_empty() {
            continue;
        }
        data.icons.insert(id, StatusIconEntry { image, name });
    }
    for (id, image) in images {
        if image.is_empty() {
            continue;
        }
        data.icons.insert(
            id,
            StatusIconEntry {
                image,
                name: String::new(),
            },
        );
    }
    Ok(data)
}

/// `StateIconImgList` is `{ [priority] = { [efst_id] = "NAME.TGA" } }`. Flatten the
/// priority buckets into a single `efst id -> image` map (image names EUC-KR decoded,
/// original case preserved).
fn collect_images(lua: &mlua::Lua) -> anyhow::Result<std::collections::BTreeMap<u32, String>> {
    let list = lua
        .globals()
        .get::<mlua::Table>("StateIconImgList")
        .map_err(lua_err)?;

    let mut images = std::collections::BTreeMap::new();
    for bucket in list.pairs::<mlua::Value, mlua::Table>() {
        let (_priority, inner) = bucket.map_err(lua_err)?;
        for pair in inner.pairs::<mlua::Value, mlua::String>() {
            let (key, image) = pair.map_err(lua_err)?;
            if let Some(id) = efst_id(&key) {
                images.insert(id, decode_euckr(image.as_bytes().as_ref()));
            }
        }
    }
    Ok(images)
}

/// `StateIconList[efst_id] = { descript = { { "Name", color }, ... } }`. The English
/// display name is the first string of the first descript row.
fn collect_names(lua: &mlua::Lua) -> anyhow::Result<std::collections::BTreeMap<u32, String>> {
    let list = lua
        .globals()
        .get::<mlua::Table>("StateIconList")
        .map_err(lua_err)?;

    let mut names = std::collections::BTreeMap::new();
    for pair in list.pairs::<mlua::Value, mlua::Table>() {
        let (key, entry) = pair.map_err(lua_err)?;
        let Some(id) = efst_id(&key) else {
            continue;
        };
        if let Some(name) = descript_name(&entry) {
            names.insert(id, name);
        }
    }
    Ok(names)
}

fn descript_name(entry: &mlua::Table) -> Option<String> {
    let descript = entry.get::<mlua::Table>("descript").ok()?;
    let first_row = descript.get::<mlua::Table>(1).ok()?;
    let name = first_row.get::<mlua::String>(1).ok()?;
    Some(decode_euckr(name.as_bytes().as_ref()))
}

fn efst_id(key: &mlua::Value) -> Option<u32> {
    match key {
        mlua::Value::Integer(id) if *id >= 0 && *id != UNKNOWN_EFST_SENTINEL => Some(*id as u32),
        mlua::Value::Number(id) if *id >= 0.0 => Some(*id as u32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_entries_with_name_or_image_drops_neither() -> anyhow::Result<()> {
        let lua = lua::new_vm_unbounded().map_err(lua_err)?;
        lua::exec_chunk(
            &lua,
            br#"
            EFST_IDs = { EFST_BLESSING = 10, EFST_PROVOKE = 0, EFST_NAMEONLY = 3, EFST_EMPTY = 5 }
            StateIconImgList = {
                [0] = {
                    [EFST_IDs.EFST_BLESSING] = "BLESSING.TGA",
                    [EFST_IDs.EFST_EMPTY] = ""
                },
                [1] = {
                    [EFST_IDs.EFST_PROVOKE] = "provoke.tga"
                }
            }
            StateIconList = {}
            StateIconList[EFST_IDs.EFST_BLESSING] = {
                descript = { { "Blessing", { 155, 202, 155 } }, { "buff" } }
            }
            StateIconList[EFST_IDs.EFST_NAMEONLY] = {
                descript = { { "Concentration" } }
            }
            "#,
        )
        .map_err(lua_err)?;

        let data = extract_status_icons(&lua)?;

        // Both name and image present.
        let blessing = data.icons.get(&10).expect("efst 10");
        assert_eq!(blessing.image, "BLESSING.TGA");
        assert_eq!(blessing.name, "Blessing");

        // Name only (no image) is KEPT with image = "".
        let name_only = data.icons.get(&3).expect("efst 3 name-only kept");
        assert_eq!(name_only.image, "");
        assert_eq!(name_only.name, "Concentration");

        // Image only (no name) is KEPT with name = "".
        let image_only = data.icons.get(&0).expect("efst 0 image-only kept");
        assert_eq!(image_only.image, "provoke.tga");
        assert_eq!(image_only.name, "");

        // Neither name nor image -> DROPPED (empty image string, no name entry).
        assert!(
            !data.icons.contains_key(&5),
            "empty-image no-name entry dropped"
        );
        Ok(())
    }

    #[test]
    fn unknown_efst_name_resolves_to_sentinel_and_is_skipped() -> anyhow::Result<()> {
        let lua = lua::new_vm_unbounded().map_err(lua_err)?;
        lua::exec_chunk(&lua, b"EFST_IDs = { EFST_KNOWN = 7 }").map_err(lua_err)?;
        install_efst_fallback(&lua)?;
        lua::exec_chunk(
            &lua,
            br#"
            StateIconImgList = {
                [0] = {
                    [EFST_IDs.EFST_KNOWN] = "KNOWN.TGA",
                    [EFST_IDs.EFST_MISSING] = "MISSING.TGA"
                }
            }
            StateIconList = {}
            "#,
        )
        .map_err(lua_err)?;

        let data = extract_status_icons(&lua)?;
        assert_eq!(data.icons.len(), 1);
        assert!(data.icons.contains_key(&7));
        Ok(())
    }
}
