use bevy::prelude::*;
use lifthrasir_data::{QuestData, QuestText};
use std::collections::BTreeMap;

/// Quest display text: quest id -> title, description lines and summary.
#[derive(Resource)]
pub struct QuestDb {
    quests: BTreeMap<u32, QuestText>,
}

impl QuestDb {
    pub fn from_data(data: QuestData) -> Self {
        Self {
            quests: data.quests,
        }
    }

    pub fn text(&self, quest_id: u32) -> Option<&QuestText> {
        self.quests.get(&quest_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_resolves_present_quest_and_misses_absent_one() {
        let mut data = QuestData::default();
        data.quests.insert(
            5,
            QuestText {
                title: "Ransom".to_string(),
                description: vec!["Turtle".to_string()],
                summary: "Carapace".to_string(),
            },
        );
        let db = QuestDb::from_data(data);

        assert_eq!(db.text(5).map(|t| t.title.as_str()), Some("Ransom"));
        assert!(db.text(6).is_none());
    }
}
