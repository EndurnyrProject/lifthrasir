use std::collections::HashMap;

use bevy::prelude::*;
use net_contract::events::{UnitEntered, UnitStateChanged};

use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::RenderLayer;

/// aesir `OPT1_*` body-state wire ids (`Aesir.ZoneServer.Mmo.Opt1`, the rAthena
/// `e_sc_opt1` table). Single-valued: `UnitStateChanged.body_state` carries at
/// most one of these. Stone Curse's server-side wait phase still reports
/// `:stone`, so both petrify ids render identically.
const OPT1_STONE: u32 = 1;
const OPT1_FREEZE: u32 = 2;
const OPT1_STONEWAIT: u32 = 6;

/// Frozen-solid tint (ice blue), multiplied into every sprite layer material.
const ICE_BLUE: Color = Color::srgb(0.5, 0.75, 1.0);
/// Petrified tint (stone gray).
const STONE_GRAY: Color = Color::srgb(0.5, 0.5, 0.5);

/// Colour multiplied into a unit's sprite layers while a body-state pose is
/// active. Read every frame by [`apply_body_state_tint`]; its absence means the
/// layers render at their natural colour.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BodyStateTint(pub Color);

/// Holds a unit's sprite animation on the frame that was showing at `at_ms`.
/// Read by the body layer sync, which feeds this frozen timestamp to the
/// frame-index computation instead of the live clock. Deliberately not an
/// `AnimationState` variant so the behaviour state machine (Hit/Dead) never
/// fights it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AnimationPaused {
    pub at_ms: u32,
}

/// Latest `body_state` for units whose entity has not registered yet, keyed by
/// unit_id. On map load the local player gets a self-directed `UnitStateChange`
/// that can arrive before its entity registers in `EntityRegistry`; without this
/// a frozen local player would render unfrozen. Retried every frame until the
/// unit resolves. Latest value wins, so a freeze-then-clear pair collapses
/// correctly. Mirrors `PendingStatusParams`.
#[derive(Resource, Default)]
pub struct PendingBodyStates(HashMap<u32, u32>);

/// Maps a body-state wire id to its tint, or `None` for states with no visual
/// (0/none, stun, sleep, ...).
fn body_state_tint(body_state: u32) -> Option<Color> {
    match body_state {
        OPT1_FREEZE => Some(ICE_BLUE),
        OPT1_STONE | OPT1_STONEWAIT => Some(STONE_GRAY),
        _ => None,
    }
}

/// Reconciles a unit's freeze/stone visuals with its current `body_state`:
/// insert tint + animation pause when a petrify/freeze pose is active, remove
/// both otherwise. Consumes both channels that carry `body_state` — live
/// `UnitStateChanged` toggles and the spawn-time `UnitEntered` for units that
/// enter view already frozen (no follow-up state change arrives in that case) —
/// so it is ordered after entity spawning to resolve the entered unit.
pub fn body_state_visuals(
    time: Res<Time>,
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingBodyStates>,
    mut commands: Commands,
) {
    let at_ms = (time.elapsed_secs() * 1000.0) as u32;

    for event in state_changes.read() {
        match registry.get_entity(event.unit_id) {
            Some(entity) => apply_body_state(&mut commands, entity, event.body_state, at_ms),
            None => {
                pending.0.insert(event.unit_id, event.body_state);
            }
        }
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_body_state(&mut commands, entity, event.body_state, at_ms);
    }

    pending.0.retain(|&unit_id, &mut body_state| {
        let Some(entity) = registry.get_entity(unit_id) else {
            return true;
        };
        apply_body_state(&mut commands, entity, body_state, at_ms);
        false
    });
}

fn apply_body_state(commands: &mut Commands, entity: Entity, body_state: u32, at_ms: u32) {
    match body_state_tint(body_state) {
        Some(color) => {
            commands
                .entity(entity)
                .insert((BodyStateTint(color), AnimationPaused { at_ms }));
        }
        None => {
            commands
                .entity(entity)
                .remove::<(BodyStateTint, AnimationPaused)>();
        }
    }
}

/// Multiplies each sprite layer's material `base_color` by its parent unit's
/// [`BodyStateTint`], or resets it to white when the unit has none. This rides
/// the same per-frame path as the layer texture write, because those materials
/// are rewritten unconditionally every frame (retained-phase re-queue) — a
/// one-shot tint write would be lost. Covers every layer uniformly (body, head,
/// weapon, headgear, cart) since they are all `RenderLayer` children of the unit.
pub fn apply_body_state_tint(
    mut materials: ResMut<Assets<StandardMaterial>>,
    layers: Query<(&MeshMaterial3d<StandardMaterial>, &ChildOf), With<RenderLayer>>,
    tints: Query<&BodyStateTint>,
) {
    for (material_handle, child_of) in &layers {
        let desired = tints
            .get(child_of.parent())
            .map_or(Color::WHITE, |tint| tint.0);

        // Read before mutating: `get_mut` marks the material changed (a retained-
        // phase re-queue) every call, so touch it only when the colour actually
        // differs. The steady state (untinted white) then costs a read, not a
        // write, keeping this off the per-frame asset-mutation hot path.
        if materials.get(&material_handle.0).map(|m| m.base_color) == Some(desired) {
            continue;
        }

        let Some(mut material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        material.base_color = desired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<UnitStateChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingBodyStates>()
            .add_systems(Update, body_state_visuals);
        app
    }

    fn register(app: &mut App, gid: u32, entity: Entity) {
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
    }

    fn emit_state(app: &mut App, unit_id: u32, body_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id,
                body_state,
                health_state: 0,
                effect_state: 0,
                virtue: 0,
            });
        app.update();
    }

    fn emit_entered(app: &mut App, gid: u32, body_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(UnitEntered {
                gid,
                aid: 0,
                object_type: 0,
                job: 0,
                x: 0,
                y: 0,
                dir: 0,
                speed: 0,
                hp: 0,
                max_hp: 0,
                clevel: 0,
                body_state,
                health_state: 0,
                effect_state: 0,
                head: 0,
                weapon: 0,
                shield: 0,
                accessory: 0,
                accessory2: 0,
                accessory3: 0,
                head_palette: 0,
                body_palette: 0,
                head_dir: 0,
                robe: 0,
                guild_id: 0,
                guild_name: String::new(),
                emblem_id: 0,
                sex: 0,
                is_boss: false,
                name: String::new(),
                moving: false,
                dst_x: 0,
                dst_y: 0,
                move_start_time: 0,
            });
        app.update();
    }

    #[test]
    fn freeze_inserts_tint_and_pause() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);

        let tint = app.world().get::<BodyStateTint>(unit);
        assert_eq!(tint, Some(&BodyStateTint(ICE_BLUE)));
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
    }

    #[test]
    fn stone_inserts_gray_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_STONE);

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(STONE_GRAY))
        );
    }

    #[test]
    fn stonewait_inserts_gray_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_STONEWAIT);

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(STONE_GRAY))
        );
    }

    #[test]
    fn clearing_body_state_removes_both() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);
        assert!(app.world().get::<BodyStateTint>(unit).is_some());

        emit_state(&mut app, 7, 0);
        assert!(app.world().get::<BodyStateTint>(unit).is_none());
        assert!(app.world().get::<AnimationPaused>(unit).is_none());
    }

    #[test]
    fn unit_entered_frozen_applies_at_spawn() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_entered(&mut app, 7, OPT1_FREEZE);

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(ICE_BLUE))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
    }

    #[test]
    fn unresolved_unit_does_not_touch_other_units() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 999, OPT1_FREEZE);

        assert!(app.world().get::<BodyStateTint>(unit).is_none());
        assert!(app.world().get::<AnimationPaused>(unit).is_none());
    }

    #[test]
    fn state_before_registration_applies_after_it() {
        let mut app = app();

        // Freeze arrives while the unit's entity has not registered yet (the
        // local-player zone-in race): buffered, not dropped.
        emit_state(&mut app, 7, OPT1_FREEZE);

        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);
        app.update();

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(ICE_BLUE))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
    }
}
