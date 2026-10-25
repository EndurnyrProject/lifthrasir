use bevy::prelude::*;
use lifthrasir_data::Visual;
use net_contract::dto::{SkillUnitGroupState, SkillUnitPhase};
use net_contract::events::{SkillUnitSnapshotReceived, SkillUnitSpawned};

use crate::domain::effects::triggers::ground_world_position;
use crate::domain::skill_units::spawn::grid_coord;
use crate::domain::world::components::CurrentMapAltitude;
use lifthrasir_assets::catalogs::effect::EffectCatalog;
use lifthrasir_assets::loaders::RoAltitudeAsset;
use lifthrasir_runtime::audio::events::PlaySkillSfx;
use lifthrasir_runtime::effects::{EffectAnchor, playback::spawn_effect};
use lifthrasir_runtime::effects::{descriptor_tint, load_str_effect};

#[allow(clippy::too_many_arguments)]
pub fn spawn_trap_detonation(
    mut spawned: MessageReader<SkillUnitSpawned>,
    mut snapshots: MessageReader<SkillUnitSnapshotReceived>,
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    mut sfx: MessageWriter<PlaySkillSfx>,
    map_altitude: Option<Res<CurrentMapAltitude>>,
    altitude_assets: Option<Res<Assets<RoAltitudeAsset>>>,
) {
    let groups = spawned
        .read()
        .map(|event| &event.group)
        .chain(snapshots.read().flat_map(|snapshot| &snapshot.groups));

    for group in groups {
        spawn_group_detonation(
            group,
            &mut commands,
            catalog.as_deref(),
            &asset_server,
            &mut sfx,
            map_altitude.as_deref(),
            altitude_assets.as_deref(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_group_detonation(
    group: &SkillUnitGroupState,
    commands: &mut Commands,
    catalog: Option<&EffectCatalog>,
    asset_server: &AssetServer,
    sfx: &mut MessageWriter<PlaySkillSfx>,
    map_altitude: Option<&CurrentMapAltitude>,
    altitude_assets: Option<&Assets<RoAltitudeAsset>>,
) {
    // ACTIVE and OPENING units have not triggered yet: OPENING means the unit
    // exists but cannot accept entry until it finishes arming.
    if matches!(
        group.phase,
        SkillUnitPhase::Active | SkillUnitPhase::Opening
    ) {
        return;
    }
    let Some(descriptor) = catalog.and_then(|catalog| catalog.skill(group.skill_id)) else {
        return;
    };
    let Some(trigger) = &descriptor.on_trigger else {
        return;
    };
    let Visual::Str(name) = &trigger.visual else {
        unreachable!("effect catalog validates trigger visuals as STR")
    };
    let (Some(x), Some(y)) = (grid_coord(group.center_x), grid_coord(group.center_y)) else {
        return;
    };
    let position = ground_world_position(x, y, map_altitude, altitude_assets);
    let effect = spawn_effect(
        commands,
        load_str_effect(asset_server, name),
        EffectAnchor::Position(position),
        false,
        descriptor_tint(descriptor),
        None,
    );
    if let Some(sound) = &trigger.sound {
        sfx.write(PlaySkillSfx {
            emitter: effect,
            sound: sound.clone(),
        });
    }
}
