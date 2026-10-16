use bevy::prelude::*;
use lifthrasir_assets::animation::{
    PendingAnimation, PendingAnimations,
    tags::{LAYER_BODY, LAYER_HEAD},
};

fn completion(entity: Entity, layer_tag: moonshine_tag::Tag) -> PendingAnimation {
    PendingAnimation {
        sprite_handle: Handle::default(),
        action_handle: Handle::default(),
        palette_handle: None,
        layer_tag,
        callback_entity: Some(entity),
    }
}

#[test]
fn layer_claims_leave_other_completions_and_deferred_work_intact() {
    let entity = World::new().spawn_empty().id();
    let mut queue = PendingAnimations::default();
    queue.defer_completed(vec![
        (completion(entity, LAYER_BODY), Handle::default()),
        (completion(entity, LAYER_HEAD), Handle::default()),
    ]);
    let body = queue.take_completed_for_layer(LAYER_BODY);
    assert_eq!(body.len(), 1);
    queue.defer_completed(body);
    assert_eq!(queue.take_completed_for_layer(LAYER_HEAD).len(), 1);
    assert_eq!(queue.take_completed_for_layer(LAYER_BODY).len(), 1);
    assert!(queue.take_completed_where(|_| true).is_empty());
}

#[test]
fn superseding_a_layer_cancels_queued_and_completed_work_for_only_that_target() {
    let mut world = World::new();
    let target = world.spawn_empty().id();
    let other = world.spawn_empty().id();
    let mut queue = PendingAnimations::default();
    queue.request(
        Handle::default(),
        Handle::default(),
        None,
        LAYER_BODY,
        Some(target),
    );
    queue.defer_completed(vec![
        (completion(target, LAYER_BODY), Handle::default()),
        (completion(target, LAYER_HEAD), Handle::default()),
        (completion(other, LAYER_BODY), Handle::default()),
    ]);
    queue.discard_for(target, LAYER_BODY);
    assert!(!queue.has_pending());
    let remaining = queue.take_completed_where(|_| true);
    assert_eq!(remaining.len(), 2);
    assert!(
        remaining
            .iter()
            .all(|(request, _)| request.callback_entity != Some(target)
                || request.layer_tag != LAYER_BODY)
    );
}
