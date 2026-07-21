use std::collections::HashMap;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};

use net_contract::events::SkillCooldownSet;

/// Per-skill post-cast cooldowns, populated from `SkillCooldownSet` and ticked
/// down with `Time`. `resolve_skill_cast` gates on this; Spec B's cooldown
/// sweep reads `remaining_secs`.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct SkillCooldownTracker {
    remaining: HashMap<u32, Timer>,
}

impl SkillCooldownTracker {
    pub fn is_on_cooldown(&self, id: u32) -> bool {
        self.remaining.contains_key(&id)
    }

    pub fn remaining_secs(&self, id: u32) -> Option<f32> {
        self.remaining.get(&id).map(Timer::remaining_secs)
    }
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn apply_skill_cooldown(
    mut events: MessageReader<SkillCooldownSet>,
    mut tracker: ResMut<SkillCooldownTracker>,
) {
    for event in events.read() {
        tracker.remaining.insert(
            event.skill_id,
            Timer::from_seconds(event.tick as f32 / 1000.0, TimerMode::Once),
        );
    }
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn tick_skill_cooldowns(time: Res<Time>, mut tracker: ResMut<SkillCooldownTracker>) {
    let delta = time.delta();
    tracker.remaining.retain(|_, timer| {
        timer.tick(delta);
        !timer.is_finished()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn cooldown_set_marks_on_cooldown_then_clears_when_elapsed() {
        let mut app = App::new();
        app.add_message::<SkillCooldownSet>()
            .init_resource::<Time>()
            .init_resource::<SkillCooldownTracker>()
            .add_systems(Update, (apply_skill_cooldown, tick_skill_cooldowns).chain());

        app.world_mut()
            .resource_mut::<Messages<SkillCooldownSet>>()
            .write(SkillCooldownSet {
                skill_id: 7,
                tick: 1000,
            });

        app.update();

        assert!(
            app.world()
                .resource::<SkillCooldownTracker>()
                .is_on_cooldown(7)
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1100));
        app.update();

        let tracker = app.world().resource::<SkillCooldownTracker>();
        assert!(!tracker.is_on_cooldown(7));
        assert_eq!(tracker.remaining_secs(7), None);
    }

    #[test]
    fn remaining_secs_decreases_across_two_ticks() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<SkillCooldownTracker>()
            .add_systems(Update, tick_skill_cooldowns);

        app.world_mut()
            .resource_mut::<SkillCooldownTracker>()
            .remaining
            .insert(7, Timer::from_seconds(5.0, TimerMode::Once));

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1000));
        app.update();
        let first = app
            .world()
            .resource::<SkillCooldownTracker>()
            .remaining_secs(7)
            .expect("still on cooldown");

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1000));
        app.update();
        let second = app
            .world()
            .resource::<SkillCooldownTracker>()
            .remaining_secs(7)
            .expect("still on cooldown");

        assert!(second < first);
    }
}
