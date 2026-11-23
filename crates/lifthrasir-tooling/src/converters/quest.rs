use crate::OperationReport;
use crate::converters::read_system_en;
use crate::encoding::decode_euckr;
use crate::grf_vfs::GrfVfs;
use crate::lua;
use anyhow::Context;
use lifthrasir_data::{QuestData, QuestText};
use std::path::Path;

const QUESTS_PATH: &str = "OngoingQuests.lub";

pub fn run(_vfs: &GrfVfs, out: &Path) -> anyhow::Result<OperationReport> {
    let src = read_system_en(QUESTS_PATH)?;
    let quest_data = parse_quest_info(&src)?;

    let ron = ron::ser::to_string_pretty(&quest_data, ron::ser::PrettyConfig::default())?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let dest = out.join("quest_data.ron");
    std::fs::write(&dest, ron).with_context(|| format!("writing {}", dest.display()))?;

    let mut report = OperationReport {
        outputs: vec![dest],
        ..Default::default()
    };
    report.info(format!(
        "quest_data.ron: {} quests",
        quest_data.quests.len()
    ));
    Ok(report)
}

/// Reads the global `QuestInfoList` table defined by an `OngoingQuests.lub` chunk.
pub fn parse_quest_info(src: &[u8]) -> anyhow::Result<QuestData> {
    let lua = lua::new_vm_unbounded().map_err(lua_err)?;
    lua::exec_chunk(&lua, src).map_err(lua_err)?;

    let tbl: mlua::Table = lua.globals().get("QuestInfoList").map_err(lua_err)?;

    let mut quest_data = QuestData::default();
    for pair in tbl.pairs::<mlua::Value, mlua::Table>() {
        let (key, sub) = pair.map_err(lua_err)?;
        if let Some(id) = quest_id(&key) {
            quest_data.quests.insert(id, parse_quest(&sub));
        }
    }
    Ok(quest_data)
}

fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn quest_id(key: &mlua::Value) -> Option<u32> {
    match key {
        mlua::Value::Integer(id) => u32::try_from(*id).ok(),
        mlua::Value::Number(id) if id.fract() == 0.0 => u32::try_from(*id as i64).ok(),
        _ => None,
    }
}

fn parse_quest(sub: &mlua::Table) -> QuestText {
    QuestText {
        title: euckr_field(sub, "Title"),
        description: euckr_lines(sub, "Description"),
        summary: euckr_field(sub, "Summary"),
    }
}

fn euckr_field(sub: &mlua::Table, key: &str) -> String {
    match sub.get::<Option<mlua::LuaString>>(key) {
        Ok(Some(s)) => decode_euckr(s.as_bytes().as_ref()),
        _ => String::default(),
    }
}

fn euckr_lines(sub: &mlua::Table, key: &str) -> Vec<String> {
    let Ok(Some(table)) = sub.get::<Option<mlua::Table>>(key) else {
        return Vec::new();
    };
    table
        .sequence_values::<mlua::LuaString>()
        .filter_map(Result::ok)
        .map(|s| decode_euckr(s.as_bytes().as_ref()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_two_line_description_and_summary() {
        let src = br#"QuestInfoList = {
            [101] = {
                Title = "Introduction",
                Description = { "Go to the hall.", "Speak to the professor." },
                Summary = "Check in"
            }
        }"#;

        let data = parse_quest_info(src).unwrap();

        assert_eq!(
            data.quests[&101],
            QuestText {
                title: "Introduction".into(),
                description: vec!["Go to the hall.".into(), "Speak to the professor.".into()],
                summary: "Check in".into(),
            }
        );
    }

    #[test]
    fn missing_summary_and_description_default_to_empty() {
        let src = br#"QuestInfoList = {
            [1] = { Title = "No summary", Description = { "Only line" } },
            [2] = { Title = "No description", Summary = "Short" }
        }"#;

        let data = parse_quest_info(src).unwrap();

        assert_eq!(data.quests[&1].summary, "");
        assert_eq!(data.quests[&1].description, vec!["Only line"]);
        assert!(data.quests[&2].description.is_empty());
        assert_eq!(data.quests[&2].summary, "Short");
    }

    #[test]
    fn decodes_euckr_high_bytes() {
        let src = b"QuestInfoList = { [7] = { Title = \"\\195\\202\" } }";

        let data = parse_quest_info(src).unwrap();

        assert_eq!(data.quests[&7].title, decode_euckr(&[0xC3, 0xCA]));
    }

    #[test]
    fn skips_non_integer_keys() {
        let src = br#"QuestInfoList = {
            [5] = { Title = "Kept" },
            ["note"] = { Title = "Dropped" },
            [-3] = { Title = "Negative" }
        }"#;

        let data = parse_quest_info(src).unwrap();

        assert_eq!(data.quests.keys().copied().collect::<Vec<_>>(), vec![5]);
    }
}
