use bevy::prelude::*;

#[cfg(test)]
mod tests;
use net_contract::commands::{AttackRequested, PickupRequested, TalkToNpc};

use crate::domain::entities::components::NetworkEntity;
use crate::domain::entities::hover::{
    CurrentlyHoveredEntity, EntityHoverEntered, EntityHoverExited, HoveredEntity,
};
use crate::domain::entities::markers::{Mob, Npc};
use crate::domain::entities::types::ObjectType;
use crate::domain::input::terrain_raycast::TerrainRaycastCache;
use crate::domain::input::{CursorChangeRequest, CursorType, LockedTarget, TargetingMode};
use crate::domain::item_drop::HoveredFloorItem;
use crate::domain::item_drop::components::FloorItem;
use crate::domain::item_drop::pickup::{PendingPickups, PickupInfo};
use crate::domain::skill::{CastTarget, SkillCastResolved};

type PickRoots<'w, 's> = Query<'w, 's, (), Or<(With<NetworkEntity>, With<FloorItem>)>>;

/// Sprites are direct children; glTF primitives may be nested several nodes below their owner.
fn pick_root(child: Entity, child_of: &Query<&ChildOf>, roots: &PickRoots) -> Entity {
    let fallback = child_of.get(child).map(|c| c.parent()).unwrap_or(child);
    let mut entity = child;
    loop {
        if roots.contains(entity) {
            return entity;
        }
        let Ok(parent) = child_of.get(entity) else {
            return fallback;
        };
        entity = parent.parent();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn on_sprite_over(
    over: On<Pointer<Over>>,
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    roots: PickRoots,
    nets: Query<&NetworkEntity>,
    kinds: Query<(Has<Mob>, Has<Npc>, Has<FloorItem>)>,
    mut hovered: ResMut<CurrentlyHoveredEntity>,
    mut hovered_item: ResMut<HoveredFloorItem>,
    mut cursor: MessageWriter<CursorChangeRequest>,
) {
    let root = pick_root(over.entity, &child_of, &roots);
    hovered.entity = Some(root);

    let net = nets.get(root).ok();
    if let Some(net) = net {
        commands.entity(root).try_insert(HoveredEntity);
        commands.trigger(EntityHoverEntered {
            entity: root,
            entity_id: net.aid,
        });
    } else if kinds.get(root).map(|(_, _, item)| item).unwrap_or(false) {
        hovered_item.0 = Some(root);
    }

    let (is_mob, is_npc, is_item) = kinds.get(root).unwrap_or((false, false, false));
    let is_skill_unit = net.is_some_and(|net| net.object_type == ObjectType::SkillUnit);
    let cursor_type = if is_mob || is_skill_unit {
        CursorType::Attack
    } else if is_npc {
        CursorType::Talk
    } else if is_item {
        CursorType::Add
    } else {
        CursorType::Default
    };
    cursor.write(CursorChangeRequest::new(cursor_type));
}

#[allow(clippy::too_many_arguments)]
pub fn on_sprite_out(
    out: On<Pointer<Out>>,
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    roots: PickRoots,
    nets: Query<&NetworkEntity>,
    mut hovered: ResMut<CurrentlyHoveredEntity>,
    mut hovered_item: ResMut<HoveredFloorItem>,
    cache: Res<TerrainRaycastCache>,
    mut cursor: MessageWriter<CursorChangeRequest>,
) {
    let root = pick_root(out.entity, &child_of, &roots);

    if hovered.entity == Some(root) {
        hovered.entity = None;
    }

    if nets.get(root).is_ok() {
        commands.entity(root).try_remove::<HoveredEntity>();
        commands.trigger(EntityHoverExited { entity: root });
    }

    if hovered_item.0 == Some(root) {
        hovered_item.0 = None;
    }

    let cursor_type = if cache.is_walkable {
        CursorType::Default
    } else {
        CursorType::Impossible
    };
    cursor.write(CursorChangeRequest::new(cursor_type));
}

#[allow(clippy::too_many_arguments)]
pub fn on_sprite_click(
    click: On<Pointer<Click>>,
    child_of: Query<&ChildOf>,
    roots: PickRoots,
    nets: Query<&NetworkEntity>,
    kinds: Query<(Has<Mob>, Has<Npc>)>,
    floor_items: Query<&FloorItem>,
    mut targeting: ResMut<TargetingMode>,
    mut attacks: MessageWriter<AttackRequested>,
    mut pickups: MessageWriter<PickupRequested>,
    mut talks: MessageWriter<TalkToNpc>,
    mut skills: MessageWriter<SkillCastResolved>,
    mut locked: ResMut<LockedTarget>,
    mut pending: ResMut<PendingPickups>,
) {
    if click.event.button != PointerButton::Primary {
        return;
    }

    let root = pick_root(click.entity, &child_of, &roots);

    if let TargetingMode::AwaitingEntity { skill_id, level } = *targeting {
        if let Ok(net) = nets.get(root) {
            skills.write(SkillCastResolved {
                skill_id,
                level,
                target: CastTarget::Entity(net.gid),
            });
            *targeting = TargetingMode::Idle;
        }
        return;
    }

    if *targeting != TargetingMode::Idle {
        return;
    }

    let (is_mob, is_npc) = kinds.get(root).unwrap_or((false, false));
    let net = nets.get(root).ok();
    let is_attackable = is_mob || net.is_some_and(|net| net.object_type == ObjectType::SkillUnit);
    if is_attackable {
        if let Some(net) = net {
            attacks.write(AttackRequested { target_id: net.gid });
            *locked = LockedTarget { gid: Some(net.gid) };
        }
        return;
    }

    if is_npc {
        if let Ok(net) = nets.get(root) {
            talks.write(TalkToNpc { npc_id: net.gid });
        }
        return;
    }

    if let Ok(item) = floor_items.get(root) {
        pickups.write(PickupRequested {
            ground_id: item.ground_id,
        });
        pending.0.insert(
            item.ground_id,
            PickupInfo {
                nameid: item.nameid,
                amount: item.amount,
                identified: item.identified,
            },
        );
    }
}
