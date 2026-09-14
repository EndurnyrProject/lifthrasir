use super::*;
use bevy::{
    camera::{ImageRenderTarget, NormalizedRenderTarget},
    picking::{
        backend::HitData,
        pointer::{Location, PointerId},
    },
};
use std::time::Duration;

#[test]
fn nested_gltf_and_direct_sprite_picks_target_the_network_owner() {
    for kind in [ObjectType::Npc, ObjectType::Mob] {
        for nested in [false, true] {
            let mut app = App::new();
            app.init_resource::<TargetingMode>()
                .init_resource::<LockedTarget>()
                .init_resource::<PendingPickups>()
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
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Image(ImageRenderTarget {
                        handle: Handle::default(),
                        scale_factor: 1.0,
                    }),
                    position: Vec2::ZERO,
                },
                Click {
                    button: PointerButton::Primary,
                    hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                    duration: Duration::ZERO,
                    count: 1,
                },
                mesh,
            ));
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
