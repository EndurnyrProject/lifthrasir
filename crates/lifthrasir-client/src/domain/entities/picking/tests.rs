use crate::domain::entities::picking::*;
use bevy::{
    camera::{ImageRenderTarget, NormalizedRenderTarget},
    picking::{
        backend::HitData,
        pointer::{Location, PointerId},
    },
};
use std::time::Duration;

/// Build the pointer click event Bevy's picking emits for the primary button.
fn pointer_click(entity: Entity) -> PointerClick {
    PointerClick {
        entity,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Image(ImageRenderTarget {
                    handle: Handle::default(),
                    scale_factor: 1.0,
                }),
                position: Vec2::ZERO,
            },
        ),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        duration: Duration::ZERO,
        count: 1,
    }
}

#[test]
fn client_attaches_interaction_to_explicit_visual_root_mappings() {
    let mut app = App::new();
    app.init_resource::<TargetingMode>()
        .init_resource::<LockedTarget>()
        .init_resource::<PendingPickups>()
        .init_resource::<crate::domain::vending::VendingState>()
        .add_message::<AttackRequested>()
        .add_message::<PickupRequested>()
        .add_message::<TalkToNpc>()
        .add_message::<SkillCastResolved>()
        .add_observer(attach_actor_visual_observers);
    let owner = app
        .world_mut()
        .spawn((NetworkEntity::new(17, 42, ObjectType::Mob), Mob))
        .id();
    let unrelated_parent = app.world_mut().spawn_empty().id();
    let body = app
        .world_mut()
        .spawn((ChildOf(unrelated_parent), ActorVisualRoot(owner)))
        .id();
    app.world_mut().flush();
    app.world_mut().trigger(pointer_click(body));
    let messages = app.world().resource::<Messages<AttackRequested>>();
    let requests: Vec<_> = messages.iter_current_update_messages().collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].target_id, 42);
}

#[test]
fn nested_gltf_and_direct_sprite_picks_target_the_network_owner() {
    for kind in [ObjectType::Npc, ObjectType::Mob] {
        for nested in [false, true] {
            let mut app = App::new();
            app.init_resource::<TargetingMode>()
                .init_resource::<LockedTarget>()
                .init_resource::<PendingPickups>()
                .init_resource::<crate::domain::vending::VendingState>()
                .add_message::<AttackRequested>()
                .add_message::<PickupRequested>()
                .add_message::<TalkToNpc>()
                .add_message::<SkillCastResolved>();
            let owner = app.world_mut().spawn(NetworkEntity::new(17, 42, kind)).id();
            match kind {
                ObjectType::Npc => {
                    app.world_mut().entity_mut(owner).insert(Npc);
                }
                ObjectType::Mob => {
                    app.world_mut().entity_mut(owner).insert(Mob);
                }
                _ => unreachable!(),
            }
            let parent = if nested {
                app.world_mut().spawn(ChildOf(owner)).id()
            } else {
                owner
            };
            let mesh = app
                .world_mut()
                .spawn(ChildOf(parent))
                .observe(on_sprite_click)
                .id();
            app.world_mut().trigger(pointer_click(mesh));
            if kind == ObjectType::Npc {
                let messages = app.world().resource::<Messages<TalkToNpc>>();
                let requests: Vec<_> = messages.iter_current_update_messages().collect();
                assert_eq!(requests.len(), 1, "nested={nested}");
                assert_eq!(requests[0].npc_id, 42);
            } else {
                let messages = app.world().resource::<Messages<AttackRequested>>();
                let requests: Vec<_> = messages.iter_current_update_messages().collect();
                assert_eq!(requests.len(), 1, "nested={nested}");
                assert_eq!(requests[0].target_id, 42);
            }
        }
    }
}
