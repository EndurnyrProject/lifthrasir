//! Performance voice lifecycle, separate from cast artwork and attack feedback.

use bevy::prelude::*;
use lifthrasir_assets::catalogs::effect::EffectCatalog;
use lifthrasir_data::GroundAnchor;
use lifthrasir_runtime::audio::events::ControlSkillSfx;
use net_contract::events::SkillEffectShown;

use crate::domain::entities::registry::EntityRegistry;

const ADAPTATION_SKILL_ID: u32 = 304;

/// Performance fields own a cancellable voice rather than a fire-and-forget sound.
pub(super) fn is_performance(catalog: Option<&EffectCatalog>, skill_id: u32) -> bool {
    catalog
        .and_then(|catalog| catalog.skill_unit(skill_id))
        .is_some_and(|field| field.ground_anchor == GroundAnchor::Footprint)
}

/// Start or replace a successful performance's voice; Adaptation stops it even
/// while the artwork catalog is unavailable.
pub fn on_performance_skill(
    mut events: MessageReader<SkillEffectShown>,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<EffectCatalog>>,
    mut sounds: MessageWriter<ControlSkillSfx>,
) {
    let catalog = catalog.as_deref();
    for event in events.read().filter(|event| event.result == 1) {
        let Some(emitter) = registry.get_entity(event.src_id) else {
            continue;
        };
        if event.skill_id == ADAPTATION_SKILL_ID {
            sounds.write(ControlSkillSfx::Stop { emitter });
            continue;
        }
        if !is_performance(catalog, event.skill_id) {
            continue;
        }
        let Some(descriptor) = catalog.and_then(|catalog| catalog.skill(event.skill_id)) else {
            continue;
        };
        let request = match &descriptor.sound {
            Some(sound) => ControlSkillSfx::Play {
                emitter,
                sound: sound.clone(),
            },
            None => ControlSkillSfx::Stop { emitter },
        };
        sounds.write(request);
    }
}
