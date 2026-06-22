use crate::converters::read_system_en;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::{ItemData, ItemInfo};
use std::path::Path;

const ITEMINFO_PATH: &str = "LuaFiles514/itemInfo.lua";

pub fn run(_vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    let src = read_system_en(ITEMINFO_PATH)?;

    let lua = lua::new_vm_unbounded().map_err(lua_err)?;
    lua::exec_chunk(&lua, &src).map_err(lua_err)?;

    let tbl: mlua::Table = lua.globals().get("tbl").map_err(lua_err)?;

    let mut item_data = ItemData::default();
    for pair in tbl.pairs::<mlua::Value, mlua::Table>() {
        let (key, sub) = pair.map_err(lua_err)?;
        if let Some(id) = item_id(&key) {
            item_data.items.insert(id, parse_item(&sub));
        }
    }

    let ron = ron::ser::to_string_pretty(&item_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("item_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    println!("item_data.ron: {} items", item_data.items.len());
    Ok(())
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn item_id(key: &mlua::Value) -> Option<u32> {
    match key {
        mlua::Value::Integer(id) => Some(*id as u32),
        mlua::Value::Number(id) => Some(*id as u32),
        _ => None,
    }
}

fn parse_item(sub: &mlua::Table) -> ItemInfo {
    ItemInfo {
        identified_name: euckr_field(sub, "identifiedDisplayName"),
        identified_resource: euckr_field(sub, "identifiedResourceName"),
        identified_description: euckr_lines(sub, "identifiedDescriptionName"),
        unidentified_name: euckr_field(sub, "unidentifiedDisplayName"),
        unidentified_resource: euckr_field(sub, "unidentifiedResourceName"),
        unidentified_description: euckr_lines(sub, "unidentifiedDescriptionName"),
        slot_count: sub
            .get::<Option<u8>>("slotCount")
            .ok()
            .flatten()
            .unwrap_or(0),
    }
}

fn euckr_field(sub: &mlua::Table, key: &str) -> String {
    match sub.get::<Option<mlua::String>>(key) {
        Ok(Some(s)) => decode_euckr(s.as_bytes().as_ref()),
        _ => String::default(),
    }
}

fn euckr_lines(sub: &mlua::Table, key: &str) -> Vec<String> {
    let Ok(Some(table)) = sub.get::<Option<mlua::Table>>(key) else {
        return Vec::new();
    };
    table
        .sequence_values::<mlua::String>()
        .filter_map(Result::ok)
        .map(|s| decode_euckr(s.as_bytes().as_ref()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_key(src: &[u8], id: i64) -> ItemInfo {
        let lua = lua::new_vm_unbounded().unwrap();
        lua::exec_chunk(&lua, src).unwrap();
        let tbl: mlua::Table = lua.globals().get("tbl").unwrap();
        let sub: mlua::Table = tbl.get(id).unwrap();
        parse_item(&sub)
    }

    #[test]
    fn parses_names_resource_description_and_slot() {
        let src = br#"tbl = {
            [501] = {
                identifiedDisplayName = "Red Potion",
                identifiedResourceName = "RED_POTION",
                identifiedDescriptionName = { "Line one.", "Line two." },
                slotCount = 0
            },
            [2104] = {
                identifiedDisplayName = "Buckler",
                slotCount = 1
            }
        }"#;

        let red = parse_key(src, 501);
        assert_eq!(red.identified_name, "Red Potion");
        assert_eq!(red.identified_resource, "RED_POTION");
        assert_eq!(red.identified_description, vec!["Line one.", "Line two."]);
        assert_eq!(red.slot_count, 0);

        let buckler = parse_key(src, 2104);
        assert_eq!(buckler.identified_name, "Buckler");
        assert_eq!(buckler.slot_count, 1);
        assert!(buckler.identified_description.is_empty());
        assert!(buckler.identified_resource.is_empty());
    }

    #[test]
    fn decodes_euckr_resource_name() {
        let src = b"tbl = { [501] = { identifiedResourceName = \"\\195\\202\" } }";
        let item = parse_key(src, 501);
        assert_eq!(item.identified_resource, decode_euckr(&[0xC3, 0xCA]));
    }

    #[test]
    fn missing_slot_and_description_default_without_error() {
        let src = b"tbl = { [501] = { identifiedDisplayName = \"Apple\" } }";
        let item = parse_key(src, 501);
        assert_eq!(item.slot_count, 0);
        assert!(item.identified_description.is_empty());
    }
}
