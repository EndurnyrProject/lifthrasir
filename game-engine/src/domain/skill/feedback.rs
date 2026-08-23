use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;

use crate::infrastructure::item::ItemDb;
use net_contract::events::{
    ChatHeard, LearnSkillResultReceived, ProductionResult, SkillCastFailed, SkillCastFailureReason,
};

fn cast_failure_message(reason: SkillCastFailureReason) -> &'static str {
    match reason {
        SkillCastFailureReason::MissingCatalyst => {
            "You are missing a catalyst required to cast this skill."
        }
        SkillCastFailureReason::InsufficientSp => "You do not have enough SP.",
        SkillCastFailureReason::InsufficientZeny => "You do not have enough zeny.",
        SkillCastFailureReason::NoAmmo => "You need ammunition to cast this skill.",
        SkillCastFailureReason::OnCooldown => "This skill is still on cooldown.",
        SkillCastFailureReason::InvalidTarget => "You cannot cast this skill on that target.",
        SkillCastFailureReason::NotLearned => "You have not learned this skill.",
        SkillCastFailureReason::OutOfRange => "The target is out of range.",
        SkillCastFailureReason::Busy => "You cannot cast a skill right now.",
        SkillCastFailureReason::WrongWeapon => "You need a different weapon to cast this skill.",
        SkillCastFailureReason::VersusMapOnly => "This skill can only be used in versus areas.",
        SkillCastFailureReason::Unspecified => "The skill cast failed.",
    }
}

#[auto_add_system(
    plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn report_skill_cast_failure(
    mut failures: MessageReader<SkillCastFailed>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for failure in failures.read() {
        chat.write(ChatHeard {
            gid: 0,
            message: cast_failure_message(failure.reason).to_string(),
        });
    }
}

#[auto_add_system(
    plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn report_learn_skill_reject(
    mut results: MessageReader<LearnSkillResultReceived>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for result in results.read() {
        if result.ok {
            continue;
        }

        chat.write(ChatHeard {
            gid: 0,
            message: format!("Cannot learn skill (reason {})", result.reason),
        });
    }
}

/// Report the outcome of an item-production attempt (forging, brewing, ...).
/// The produced item arrives through the normal inventory updates; this is
/// only the player-facing line.
#[auto_add_system(
    plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn report_production_result(
    mut results: MessageReader<ProductionResult>,
    mut chat: MessageWriter<ChatHeard>,
    item_db: Option<Res<ItemDb>>,
) {
    for result in results.read() {
        let name = item_db
            .as_ref()
            .and_then(|db| db.name(result.item_id, true))
            .map(str::to_string)
            .unwrap_or_else(|| format!("item #{}", result.item_id));
        let message = if result.success {
            format!("You successfully created {name}.")
        } else {
            format!("You failed to create {name}.")
        };

        chat.write(ChatHeard { gid: 0, message });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn production_chat(result: ProductionResult, item_db: Option<ItemDb>) -> Vec<ChatHeard> {
        let mut app = App::new();
        app.add_message::<ProductionResult>()
            .add_message::<ChatHeard>()
            .add_systems(Update, report_production_result);
        if let Some(db) = item_db {
            app.insert_resource(db);
        }

        app.world_mut()
            .resource_mut::<Messages<ProductionResult>>()
            .write(result);
        app.update();

        app.world()
            .resource::<Messages<ChatHeard>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn item_db_with(id: u32, name: &str) -> ItemDb {
        let mut data = lifthrasir_data::ItemData::default();
        data.items.insert(
            id,
            lifthrasir_data::ItemInfo {
                identified_name: name.to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn success_names_the_produced_item() {
        let msgs = production_chat(
            ProductionResult {
                success: true,
                item_id: 1201,
            },
            Some(item_db_with(1201, "Knife")),
        );

        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].gid, 0);
        assert_eq!(msgs[0].message, "You successfully created Knife.");
    }

    #[test]
    fn failure_names_the_produced_item() {
        let msgs = production_chat(
            ProductionResult {
                success: false,
                item_id: 1201,
            },
            Some(item_db_with(1201, "Knife")),
        );

        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message, "You failed to create Knife.");
    }

    #[test]
    fn unknown_item_falls_back_to_the_id() {
        let msgs = production_chat(
            ProductionResult {
                success: true,
                item_id: 999,
            },
            None,
        );

        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message, "You successfully created item #999.");
    }

    fn run_with(result: LearnSkillResultReceived) -> Vec<ChatHeard> {
        let mut app = App::new();
        app.add_message::<LearnSkillResultReceived>()
            .add_message::<ChatHeard>()
            .add_systems(Update, report_learn_skill_reject);

        app.world_mut()
            .resource_mut::<Messages<LearnSkillResultReceived>>()
            .write(result);

        app.update();

        app.world()
            .resource::<Messages<ChatHeard>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    #[test]
    fn reject_writes_one_chat_line() {
        let msgs = run_with(LearnSkillResultReceived {
            skill_id: 40,
            ok: false,
            reason: 3,
        });

        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].gid, 0);
        assert_eq!(msgs[0].message, "Cannot learn skill (reason 3)");
    }

    #[test]
    fn ok_writes_nothing() {
        let msgs = run_with(LearnSkillResultReceived {
            skill_id: 40,
            ok: true,
            reason: 0,
        });

        assert!(msgs.is_empty());
    }

    #[test]
    fn missing_catalyst_reports_chat_feedback() {
        let mut app = App::new();
        app.add_message::<SkillCastFailed>()
            .add_message::<ChatHeard>()
            .add_systems(Update, report_skill_cast_failure);

        app.world_mut()
            .resource_mut::<Messages<SkillCastFailed>>()
            .write(SkillCastFailed {
                skill_id: 12,
                reason: SkillCastFailureReason::MissingCatalyst,
            });

        app.update();

        let chat = app.world().resource::<Messages<ChatHeard>>();
        let messages: Vec<_> = chat.iter_current_update_messages().collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].message,
            "You are missing a catalyst required to cast this skill."
        );
    }
}
