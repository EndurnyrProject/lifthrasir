//! Refreshes what the runtime spawned from a node after an edit it does not
//! observe by itself.

use bevy::prelude::*;
use lifthrasir_runtime::{
    audio::map_sounds::GltfMapSoundSpawned,
    world::{
        LifAudioEmitter, LifEffectEmitter, LifPropRef,
        emitters::GltfMapEffectSpawned,
        props::{MapModel, PropAnim},
    },
};

/// The [`EditHook`](crate::shell::undo::EditHook) of every node edit: emitters
/// lose their spawned marker so the runtime respawns their output at the
/// node's current position and values; props whose model or animation no
/// longer match their spawned visuals are rebuilt.
pub fn after_node_edit(world: &mut World, node: Entity) {
    let Ok(mut entity) = world.get_entity_mut(node) else {
        return;
    };
    if entity.contains::<LifAudioEmitter>() {
        entity.remove::<GltfMapSoundSpawned>();
    }
    if entity.contains::<LifEffectEmitter>() {
        entity.remove::<GltfMapEffectSpawned>();
    }
    if prop_visuals_stale(world, node) {
        respawn_prop_visuals(world, node);
    }
}

/// Removes `MapModel` and despawns the node's `PropAnim` children, so the
/// runtime's prop spawner rebuilds the visuals from the current `LifPropRef`.
pub fn respawn_prop_visuals(world: &mut World, node: Entity) {
    let visuals: Vec<Entity> = world
        .get::<Children>(node)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&child| world.get::<PropAnim>(child).is_some())
        .collect();
    for visual in visuals {
        world.entity_mut(visual).despawn();
    }
    world.entity_mut(node).remove::<MapModel>();
}

/// True when the prop was spawned from another model or animation than its
/// `LifPropRef` now holds. Unspawned props are never stale.
fn prop_visuals_stale(world: &World, node: Entity) -> bool {
    let (Some(prop), Some(model)) = (world.get::<LifPropRef>(node), world.get::<MapModel>(node))
    else {
        return false;
    };
    if model.filename != prop.0.model {
        return true;
    }
    world
        .get::<Children>(node)
        .into_iter()
        .flatten()
        .filter_map(|&child| world.get::<PropAnim>(child))
        .any(|anim| anim.anim_type != prop.0.anim_type || anim.anim_speed != prop.0.anim_speed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::lif::LifProp;

    fn spawned_prop(world: &mut World, model: &str) -> Entity {
        let prop = LifProp {
            model: model.into(),
            anim_type: 0,
            anim_speed: 1.0,
        };
        let node = world
            .spawn((
                LifPropRef(prop.clone()),
                MapModel {
                    filename: model.into(),
                    node_name: String::new(),
                },
            ))
            .id();
        world.spawn((
            PropAnim {
                model: prop.model,
                anim_type: prop.anim_type,
                anim_speed: prop.anim_speed,
            },
            ChildOf(node),
        ));
        node
    }

    #[test]
    fn a_prop_respawns_only_when_its_model_or_animation_changed() {
        let mut world = World::new();
        let node = spawned_prop(&mut world, "ro://models/a.glb");

        after_node_edit(&mut world, node);
        assert!(world.get::<MapModel>(node).is_some());

        world.get_mut::<LifPropRef>(node).unwrap().0.anim_speed = 2.0;
        after_node_edit(&mut world, node);
        assert!(world.get::<MapModel>(node).is_none());
        assert!(world.get::<Children>(node).is_none());
    }

    #[test]
    fn a_changed_model_respawns_the_prop() {
        let mut world = World::new();
        let node = spawned_prop(&mut world, "ro://models/a.glb");

        world.get_mut::<LifPropRef>(node).unwrap().0.model = "ro://models/b.glb".into();
        after_node_edit(&mut world, node);

        assert!(world.get::<MapModel>(node).is_none());
    }
}
