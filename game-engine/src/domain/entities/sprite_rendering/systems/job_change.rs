use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::super::components::{HeadAttachment, PendingRenderLayers, RenderLayer};
use crate::domain::entities::character::components::{CharacterData, Gender};
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::sprite::tags::LAYER_BODY;
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::job::registry::JobSpriteRegistry;
use net_contract::events::UnitSpriteChanged;

/// `LOOK_BASE`: the job/class look slot. The server broadcasts it to the
/// changing player *and* to nearby players, so this runs for the local player
/// too, unlike the equipment look types in `domain::equipment::sprite_change`.
const LOOK_BASE: u32 = 0;

type BodyLayerQuery<'w, 's> = Query<'w, 's, (Entity, &'static RenderLayer)>;

/// Rebuild the body sprite when the server changes a unit's base look (a job
/// change). The body layer child is despawned and its animation re-requested
/// from the new job's SPR/ACT; `finalize_render_layers` spawns the replacement,
/// which is why `PendingRenderLayers` goes back on the unit.
///
/// The head layer's `HeadAttachment` is dropped at the same time: it points at
/// the despawned body layer entity, and `link_head_to_body` only links heads
/// that have no attachment, so without this the head would stop following the
/// body.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::HierarchySpawn, before = super::spawn::finalize_render_layers)
)]
#[allow(clippy::too_many_arguments)]
pub fn apply_base_look_changes(
    mut commands: Commands,
    mut sprite_changes: MessageReader<UnitSpriteChanged>,
    registry: Res<EntityRegistry>,
    mut characters: Query<(&mut CharacterData, &Gender, &Children)>,
    layers: BodyLayerQuery,
    heads: Query<Entity, With<HeadAttachment>>,
    asset_server: Res<AssetServer>,
    mut pending_animations: ResMut<PendingAnimations>,
    job_registry: Option<Res<JobSpriteRegistry>>,
) {
    for change in sprite_changes.read() {
        if change.type_ != LOOK_BASE {
            continue;
        }

        let Some(entity) = registry.get_entity(change.gid) else {
            continue;
        };

        let Ok((mut character, gender, children)) = characters.get_mut(entity) else {
            continue;
        };

        let job_id = change.val as u16;
        if character.job_id == job_id {
            continue;
        }

        let Some(job_registry) = job_registry.as_deref() else {
            warn!("apply_base_look_changes: JobSpriteRegistry not available");
            continue;
        };

        let gender_byte = match gender {
            Gender::Male => 1u8,
            Gender::Female => 0u8,
        };

        let Some(body_spr_path) = job_registry.get_body_sprite_path(job_id as u32, gender_byte)
        else {
            warn!(
                "apply_base_look_changes: Unknown job_id {} for entity {:?}",
                job_id, entity
            );
            continue;
        };
        let body_act_path = body_spr_path.replace(".spr", ".act");

        character.job_id = job_id;

        for child in children.iter() {
            if layers
                .get(child)
                .is_ok_and(|(_, layer)| layer.layer == LAYER_BODY)
            {
                commands.entity(child).despawn();
                continue;
            }

            if heads.contains(child) {
                commands.entity(child).remove::<HeadAttachment>();
            }
        }

        pending_animations.request(
            asset_server.load(&body_spr_path),
            asset_server.load(&body_act_path),
            LAYER_BODY,
            Some(entity),
        );
        commands.entity(entity).insert(PendingRenderLayers);

        debug!(
            "apply_base_look_changes: Rebuilding body ({}) for entity {:?}",
            body_spr_path, entity
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::components::CharacterStats;
    use crate::domain::sprite::tags::LAYER_HEAD;
    use bevy::asset::AssetPlugin;

    const GID: u32 = 150_001;
    const NOVICE: u16 = 0;
    const SWORDMAN: u16 = 1;

    struct Fixture {
        app: App,
        character: Entity,
        body_layer: Entity,
        head_layer: Entity,
    }

    fn character_data(job_id: u16) -> CharacterData {
        CharacterData {
            name: "tester".into(),
            job_id,
            level: 1,
            experience: 0,
            stats: CharacterStats {
                str: 1,
                agi: 1,
                vit: 1,
                int: 1,
                dex: 1,
                luk: 1,
                max_hp: 40,
                current_hp: 40,
                max_sp: 11,
                current_sp: 11,
            },
            slot: 0,
        }
    }

    fn layer(app: &mut App, tag: moonshine_tag::Tag) -> Entity {
        app.world_mut()
            .spawn(RenderLayer::body(Handle::default(), tag, Vec::new()))
            .id()
    }

    fn setup() -> Fixture {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<crate::infrastructure::assets::loaders::RoSpriteAsset>()
            .init_asset::<crate::infrastructure::assets::loaders::RoActAsset>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingAnimations>()
            .insert_resource(JobSpriteRegistry::from_job_data(
                lifthrasir_data::JobData::default(),
            ))
            .add_message::<UnitSpriteChanged>()
            .add_systems(Update, apply_base_look_changes);

        let body_layer = layer(&mut app, LAYER_BODY);
        let head_layer = layer(&mut app, LAYER_HEAD);
        app.world_mut()
            .entity_mut(head_layer)
            .insert(HeadAttachment {
                body_entity: body_layer,
            });

        let character = app
            .world_mut()
            .spawn((character_data(NOVICE), Gender::Male))
            .add_children(&[body_layer, head_layer])
            .id();

        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(GID, character);

        Fixture {
            app,
            character,
            body_layer,
            head_layer,
        }
    }

    fn send(app: &mut App, type_: u32, val: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitSpriteChanged>>()
            .write(UnitSpriteChanged {
                gid: GID,
                type_,
                val,
                val2: 0,
            });
        app.update();
    }

    #[test]
    fn base_look_change_rebuilds_body_and_unlinks_head() {
        let mut f = setup();
        send(&mut f.app, LOOK_BASE, SWORDMAN as u32);

        let world = f.app.world();
        assert_eq!(
            world.get::<CharacterData>(f.character).unwrap().job_id,
            SWORDMAN
        );
        assert!(world.get_entity(f.body_layer).is_err());
        assert!(world.get::<HeadAttachment>(f.head_layer).is_none());
        assert!(world.get::<PendingRenderLayers>(f.character).is_some());
        assert!(world.resource::<PendingAnimations>().has_pending());
    }

    #[test]
    fn same_job_is_a_no_op() {
        let mut f = setup();
        send(&mut f.app, LOOK_BASE, NOVICE as u32);

        let world = f.app.world();
        assert!(world.get_entity(f.body_layer).is_ok());
        assert!(world.get::<HeadAttachment>(f.head_layer).is_some());
        assert!(!world.resource::<PendingAnimations>().has_pending());
    }

    #[test]
    fn equipment_look_types_are_ignored() {
        let mut f = setup();
        send(&mut f.app, 4, SWORDMAN as u32);

        let world = f.app.world();
        assert_eq!(
            world.get::<CharacterData>(f.character).unwrap().job_id,
            NOVICE
        );
        assert!(world.get_entity(f.body_layer).is_ok());
        assert!(!world.resource::<PendingAnimations>().has_pending());
    }

    #[test]
    fn unknown_job_leaves_the_sprite_alone() {
        let mut f = setup();
        send(&mut f.app, LOOK_BASE, 999_999);

        let world = f.app.world();
        assert_eq!(
            world.get::<CharacterData>(f.character).unwrap().job_id,
            NOVICE
        );
        assert!(world.get_entity(f.body_layer).is_ok());
        assert!(!world.resource::<PendingAnimations>().has_pending());
    }
}
