use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::super::components::{HeadAttachment, PendingRenderLayers, RenderLayer};
use crate::domain::entities::character::components::{CharacterData, Gender};
use crate::domain::entities::character::systems::OPTION_RIDING;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::sprite::tags::LAYER_BODY;
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::job::registry::JobSpriteRegistry;
use net_contract::events::UnitStateChanged;

/// The unit's body currently renders its mounted (Peco) sprite. Presence of the
/// marker *is* the rendered mount state, so repeated `effect_state` broadcasts
/// with the bit unchanged don't rebuild the body.
#[derive(Component)]
pub struct RidingPeco;

type BodyLayerQuery<'w, 's> = Query<'w, 's, (Entity, &'static RenderLayer)>;

/// Swaps a unit's body sprite between its normal and mounted (Peco) body when
/// the `OPTION_RIDING` bit of `effect_state` toggles, mirroring the rebuild in
/// `apply_base_look_changes`.
///
/// Only `UnitStateChanged` is consumed: a unit already mounted on entry gets
/// the mounted body directly from the spawn path (`riding` on
/// `EntitySpriteData::Character`), so there is no spawn/swap race to reconcile.
/// A queued-but-unfinished body request (login restore arriving while the
/// spawn-time body still loads) is superseded via
/// [`PendingAnimations::discard_for`].
///
/// Jobs without a mounted body sprite (`get_riding_body_sprite_path` is `None`)
/// keep their normal body and never get the marker, so a later unmount is a
/// no-op too.
#[auto_add_system(
    plugin = crate::domain::entities::sprite_rendering::plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::HierarchySpawn, before = super::spawn::finalize_render_layers)
)]
#[allow(clippy::too_many_arguments)]
pub fn apply_peco_mount(
    mut commands: Commands,
    mut state_changes: MessageReader<UnitStateChanged>,
    registry: Res<EntityRegistry>,
    characters: Query<(&CharacterData, &Gender, Option<&Children>)>,
    riding_markers: Query<(), With<RidingPeco>>,
    layers: BodyLayerQuery,
    heads: Query<Entity, With<HeadAttachment>>,
    asset_server: Res<AssetServer>,
    mut pending_animations: ResMut<PendingAnimations>,
    job_registry: Option<Res<JobSpriteRegistry>>,
) {
    for event in state_changes.read() {
        let desired = event.effect_state & OPTION_RIDING != 0;

        let Some(entity) = registry.get_entity(event.unit_id) else {
            continue;
        };
        if riding_markers.contains(entity) == desired {
            continue;
        }
        let Ok((character, gender, children)) = characters.get(entity) else {
            continue;
        };
        let Some(job_registry) = job_registry.as_deref() else {
            warn!("apply_peco_mount: JobSpriteRegistry not available");
            continue;
        };

        let gender_byte = match gender {
            Gender::Male => 1u8,
            Gender::Female => 0u8,
        };
        let job_id = character.job_id as u32;

        let body_spr_path = if desired {
            let Some(path) = job_registry.get_riding_body_sprite_path(job_id, gender_byte) else {
                continue;
            };
            path
        } else {
            let Some(path) = job_registry.get_body_sprite_path(job_id, gender_byte) else {
                warn!(
                    "apply_peco_mount: Unknown job_id {} for entity {:?}",
                    job_id, entity
                );
                continue;
            };
            path
        };
        let body_act_path = body_spr_path.replace(".spr", ".act");

        if let Some(children) = children {
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
        }

        pending_animations.discard_for(entity, LAYER_BODY);
        pending_animations.request(
            asset_server.load(&body_spr_path),
            asset_server.load(&body_act_path),
            None,
            LAYER_BODY,
            Some(entity),
        );

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(PendingRenderLayers);
        if desired {
            entity_commands.insert(RidingPeco);
        } else {
            entity_commands.remove::<RidingPeco>();
        }

        debug!(
            "apply_peco_mount: Rebuilding body ({}) for entity {:?}",
            body_spr_path, entity
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::components::CharacterStats;
    use crate::domain::entities::sprite_rendering::components::BodyAttachPoint;
    use crate::domain::sprite::tags::LAYER_HEAD;
    use bevy::asset::AssetPlugin;

    const GID: u32 = 150_001;
    const KNIGHT: u16 = 7;
    const NOVICE: u16 = 0;

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

    fn setup(job_id: u16) -> Fixture {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<crate::infrastructure::assets::loaders::RoSpriteAsset>()
            .init_asset::<crate::infrastructure::assets::loaders::RoActAsset>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingAnimations>()
            .insert_resource(JobSpriteRegistry::from_job_data(
                lifthrasir_data::JobData::default(),
            ))
            .add_message::<UnitStateChanged>()
            .add_systems(Update, apply_peco_mount);

        let body_layer = layer(&mut app, LAYER_BODY);
        let head_layer = layer(&mut app, LAYER_HEAD);
        app.world_mut()
            .entity_mut(body_layer)
            .insert(BodyAttachPoint::default());
        let body_instance =
            moonshine_kind::Instance::from_entity(app.world().entity(body_layer)).unwrap();
        app.world_mut()
            .entity_mut(head_layer)
            .insert(HeadAttachment {
                body_entity: body_instance,
            });

        let character = app
            .world_mut()
            .spawn((character_data(job_id), Gender::Male))
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

    /// A character that has spawned but whose body/head layers haven't been
    /// finalized yet (no `Children` component). Mirrors the login window where
    /// the riding restore can arrive before the spawn-time body finishes loading.
    fn setup_character_only(job_id: u16) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<crate::infrastructure::assets::loaders::RoSpriteAsset>()
            .init_asset::<crate::infrastructure::assets::loaders::RoActAsset>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingAnimations>()
            .insert_resource(JobSpriteRegistry::from_job_data(
                lifthrasir_data::JobData::default(),
            ))
            .add_message::<UnitStateChanged>()
            .add_systems(Update, apply_peco_mount);

        let character = app
            .world_mut()
            .spawn((character_data(job_id), Gender::Male))
            .id();

        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(GID, character);

        (app, character)
    }

    fn send(app: &mut App, effect_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id: GID,
                body_state: 0,
                health_state: 0,
                effect_state,
                virtue: 0,
            });
        app.update();
    }

    #[test]
    fn riding_bit_rebuilds_body_and_marks_unit() {
        let mut f = setup(KNIGHT);
        send(&mut f.app, OPTION_RIDING);

        let world = f.app.world();
        assert!(world.get_entity(f.body_layer).is_err());
        assert!(world.get::<HeadAttachment>(f.head_layer).is_none());
        assert!(world.get::<RidingPeco>(f.character).is_some());
        assert!(world.get::<PendingRenderLayers>(f.character).is_some());
        assert!(world.resource::<PendingAnimations>().has_pending());
    }

    #[test]
    fn repeat_riding_bit_does_not_rebuild_again() {
        let mut f = setup(KNIGHT);
        send(&mut f.app, OPTION_RIDING);
        // Simulate the finalized rebuild: a fresh body layer exists again.
        let new_body = layer(&mut f.app, LAYER_BODY);
        f.app
            .world_mut()
            .entity_mut(f.character)
            .add_children(&[new_body]);

        send(&mut f.app, OPTION_RIDING | 0x02);

        assert!(f.app.world().get_entity(new_body).is_ok());
    }

    #[test]
    fn clearing_riding_bit_restores_normal_body() {
        let mut f = setup(KNIGHT);
        send(&mut f.app, OPTION_RIDING);
        let new_body = layer(&mut f.app, LAYER_BODY);
        f.app
            .world_mut()
            .entity_mut(f.character)
            .add_children(&[new_body]);

        send(&mut f.app, 0);

        let world = f.app.world();
        assert!(world.get_entity(new_body).is_err());
        assert!(world.get::<RidingPeco>(f.character).is_none());
    }

    #[test]
    fn job_without_mounted_body_is_untouched() {
        let mut f = setup(NOVICE);
        send(&mut f.app, OPTION_RIDING);

        let world = f.app.world();
        assert!(world.get_entity(f.body_layer).is_ok());
        assert!(world.get::<RidingPeco>(f.character).is_none());
        assert!(!world.resource::<PendingAnimations>().has_pending());
    }

    #[test]
    fn unmount_without_marker_is_a_no_op() {
        let mut f = setup(KNIGHT);
        send(&mut f.app, 0);

        let world = f.app.world();
        assert!(world.get_entity(f.body_layer).is_ok());
        assert!(!world.resource::<PendingAnimations>().has_pending());
    }

    #[test]
    fn riding_bit_before_children_still_requests_mounted_body() {
        let (mut app, character) = setup_character_only(KNIGHT);
        send(&mut app, OPTION_RIDING);

        let world = app.world();
        assert!(world.get::<RidingPeco>(character).is_some());
        assert!(world.get::<PendingRenderLayers>(character).is_some());
        assert!(world.resource::<PendingAnimations>().has_pending());
    }
}
