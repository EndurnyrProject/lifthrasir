use std::collections::HashMap;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};

use net_contract::events::SkillListReceived;

/// Authoritative client mirror of the server skill tree, rebuilt wholesale on
/// every `SkillListReceived` (the server resends the full tree each time).
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct SkillTreeState {
    pub skills: HashMap<u32, SkillNode>,
}

pub struct SkillNode {
    pub level: u32,
    pub max_level: u32,
    pub upgradable: bool,
    pub requires: Vec<(u32, u32)>,
    pub req_base_level: u32,
    pub req_job_level: u32,
    pub sp: u32,
    pub range: u32,
    pub inf_type: u32,
    pub job_id: u32,
    pub splash_radius: u16,
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn apply_skill_list(
    mut events: MessageReader<SkillListReceived>,
    mut tree: ResMut<SkillTreeState>,
) {
    let Some(latest) = events.read().last() else {
        return;
    };

    tree.skills = latest
        .skills
        .iter()
        .map(|s| {
            (
                s.skill_id,
                SkillNode {
                    level: s.level,
                    max_level: s.max_level,
                    upgradable: s.upgradable,
                    requires: s.requires.clone(),
                    req_base_level: s.req_base_level,
                    req_job_level: s.req_job_level,
                    sp: s.sp,
                    range: s.range,
                    inf_type: s.type_,
                    job_id: s.job_id,
                    splash_radius: s.splash_radius,
                },
            )
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::events::ZoneSkillInfo;

    fn skill(skill_id: u32, type_: u32, job_id: u32) -> ZoneSkillInfo {
        skill_with_splash(skill_id, type_, job_id, 0)
    }

    fn skill_with_splash(
        skill_id: u32,
        type_: u32,
        job_id: u32,
        splash_radius: u16,
    ) -> ZoneSkillInfo {
        ZoneSkillInfo {
            skill_id,
            type_,
            level: 1,
            sp: 10,
            range: 3,
            name: "Test".to_string(),
            upgradable: true,
            max_level: 5,
            requires: vec![(1, 1)],
            req_base_level: 10,
            req_job_level: 5,
            job_id,
            splash_radius,
        }
    }

    #[test]
    fn apply_skill_list_rebuilds_tree() {
        let mut app = App::new();
        app.add_message::<SkillListReceived>()
            .init_resource::<SkillTreeState>()
            .add_systems(Update, apply_skill_list);

        app.world_mut()
            .resource_mut::<Messages<SkillListReceived>>()
            .write(SkillListReceived {
                skills: vec![skill(40, 1, 7), skill_with_splash(41, 16, 7, 2)],
            });

        app.update();

        let tree = app.world().resource::<SkillTreeState>();
        assert_eq!(tree.skills.len(), 2);

        let node = tree.skills.get(&40).expect("skill 40 present");
        assert_eq!(node.inf_type, 1);
        assert_eq!(node.max_level, 5);
        assert_eq!(node.requires, vec![(1, 1)]);
        assert_eq!(node.req_base_level, 10);
        assert_eq!(node.req_job_level, 5);
        assert_eq!(node.job_id, 7);
        assert_eq!(node.splash_radius, 0);

        let node41 = tree.skills.get(&41).expect("skill 41 present");
        assert_eq!(node41.inf_type, 16);
        assert_eq!(node41.splash_radius, 2);
    }
}
