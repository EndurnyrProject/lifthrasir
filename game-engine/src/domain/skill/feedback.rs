use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;

use crate::infrastructure::networking::zone_messages::{ChatHeard, LearnSkillResultReceived};

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
}
