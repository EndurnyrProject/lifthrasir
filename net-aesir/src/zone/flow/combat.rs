use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::combat::{
    cast_cancel, damage_dealt, ground_skill, knockback, learn_skill_result, skill_casting,
    skill_cooldown, skill_damage, skill_effect, skill_list, special_effect,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{
    CastCancelled, DamageReceived, GroundSkillPlaced, KnockedBack, LearnSkillResultReceived,
    SkillCastStarted, SkillCooldownSet, SkillDamageReceived, SkillEffectShown, SkillListReceived,
    SpecialEffectShown,
};

/// Drains combat and skill bodies. These span the gameplay, world, and bulk
/// channels, so the match is on the `Body` variant directly, not the channel.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
#[allow(clippy::too_many_arguments)]
pub fn zone_drain_combat(
    mut incoming: MessageReader<IncomingMessage>,
    mut damage: MessageWriter<DamageReceived>,
    mut knocked: MessageWriter<KnockedBack>,
    mut cast_started: MessageWriter<SkillCastStarted>,
    mut skill_dmg: MessageWriter<SkillDamageReceived>,
    mut skill_fx: MessageWriter<SkillEffectShown>,
    mut cancelled: MessageWriter<CastCancelled>,
    mut cooldown: MessageWriter<SkillCooldownSet>,
    mut ground: MessageWriter<GroundSkillPlaced>,
    mut skills: MessageWriter<SkillListReceived>,
    mut learn_result: MessageWriter<LearnSkillResultReceived>,
    mut special_fx: MessageWriter<SpecialEffectShown>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::DamageDealt(d) => {
                damage.write(damage_dealt(d));
            }
            Body::Knockback(k) => {
                knocked.write(knockback(k));
            }
            Body::SkillCasting(s) => {
                cast_started.write(skill_casting(s));
            }
            Body::SkillDamage(s) => {
                skill_dmg.write(skill_damage(s));
            }
            Body::SkillEffect(s) => {
                skill_fx.write(skill_effect(s));
            }
            Body::CastCancel(c) => {
                cancelled.write(cast_cancel(c));
            }
            Body::SkillCooldown(s) => {
                cooldown.write(skill_cooldown(s));
            }
            Body::GroundSkill(g) => {
                ground.write(ground_skill(g));
            }
            Body::SkillList(l) => {
                skills.write(skill_list(l));
            }
            Body::LearnSkillResult(r) => {
                learn_result.write(learn_skill_result(r));
            }
            Body::SpecialEffect(s) => {
                special_fx.write(special_effect(s));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<DamageReceived>()
            .add_message::<KnockedBack>()
            .add_message::<SkillCastStarted>()
            .add_message::<SkillDamageReceived>()
            .add_message::<SkillEffectShown>()
            .add_message::<CastCancelled>()
            .add_message::<SkillCooldownSet>()
            .add_message::<GroundSkillPlaced>()
            .add_message::<SkillListReceived>()
            .add_message::<LearnSkillResultReceived>()
            .add_message::<SpecialEffectShown>()
            .add_systems(Update, zone_drain_combat);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn damage_dealt_preserves_negative_sign() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::DamageDealt(net::DamageDealt {
                src_id: 1,
                target_id: 2,
                damage: -50,
                r#type: 4,
                damage2: -25,
                ..Default::default()
            }),
        )]);

        let damage = app.world().resource::<Messages<DamageReceived>>();
        let events: Vec<_> = damage.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].damage, -50);
        assert_eq!(events[0].type_, 4);
        assert_eq!(events[0].damage2, -25);
    }

    #[test]
    fn ground_skill_on_world_channel_still_drains() {
        use crate::channels::WORLD;
        let app = drain(vec![(
            WORLD,
            Body::GroundSkill(net::GroundSkill {
                skill_id: 5,
                src_id: 1,
                ..Default::default()
            }),
        )]);

        let ground = app.world().resource::<Messages<GroundSkillPlaced>>();
        assert_eq!(ground.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn skill_list_on_bulk_channel_still_drains() {
        use crate::channels::BULK;
        let app = drain(vec![(BULK, Body::SkillList(net::SkillList::default()))]);

        let skills = app.world().resource::<Messages<SkillListReceived>>();
        assert_eq!(skills.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn special_effect_drains_to_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::SpecialEffect(net::SpecialEffect {
                source_id: 150001,
                effect_id: 42,
            }),
        )]);

        let events = app.world().resource::<Messages<SpecialEffectShown>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].source_id, 150001);
        assert_eq!(drained[0].effect_id, 42);
    }
}
