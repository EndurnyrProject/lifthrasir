use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;

use net_contract::events::{
    ChatHeard, LearnSkillResultReceived, SkillCastFailed, SkillCastFailureReason,
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
        SkillCastFailureReason::Unspecified => "The skill cast failed.",
    }
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
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
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
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

#[cfg(test)]
mod tests {
    use super::*;

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
