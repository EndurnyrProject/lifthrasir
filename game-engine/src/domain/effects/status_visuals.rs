use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::prelude::*;
use net_contract::events::{StatusEffectChanged, UnitEntered, UnitStateChanged};

use super::components::EffectAnchor;
use super::systems::spawn_effect;
use super::triggers::{descriptor_tint, load_effect};
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::RenderLayer;
use crate::infrastructure::effect::StatusEffectCatalog;

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

/// aesir `e_option` (`OPTION_*`) sprite effect-state bit for Sight
/// (`Aesir.ZoneServer.Mmo.Option`, `:sight => 1`; MG_SIGHT's `option: :sight`
/// applies `SC_SIGHT` with this bit). Matches rAthena `OPTION_SIGHT`.
const OPTION_SIGHT: u32 = 1;

/// Orbit radius, in world units, of the Sight placeholder visual.
const SIGHT_ORBIT_RADIUS: f32 = 2.0;
/// Full revolution period of the Sight placeholder visual.
const SIGHT_ORBIT_PERIOD_SECS: f32 = 2.0;
/// Vertical offset lifting the orbit above the unit's feet; world up is `-Y`.
const SIGHT_ORBIT_LIFT: f32 = -2.5;
/// Placeholder colour for the Sight orbit quad.
const SIGHT_ORBIT_COLOR: Color = Color::srgb(1.0, 0.9, 0.35);

/// Marks the small orbiting quad spawned as a child of a unit with the Sight
/// option bit set. There is no `sight.str` asset in the GRF (verified when
/// filling the effect catalog), so this is a minimal procedural stand-in
/// circling the unit; read every frame by [`orbit_sight_visuals`]. Task 14's
/// hanabi emitters may replace it with something fancier.
#[derive(Component, Debug, Clone, Copy)]
pub struct SightOrbit;

/// Latest `effect_state` for units whose entity has not registered yet, keyed
/// by unit_id. Same shape and rationale as [`PendingBodyStates`].
#[derive(Resource, Default)]
pub struct PendingEffectStates(HashMap<u32, u32>);

/// Reconciles a unit's Sight orbit visual with its current `effect_state`:
/// spawns the orbit child when the option bit is set and none exists yet,
/// despawns it when the bit clears. Consumes both channels that carry
/// `effect_state` — live `UnitStateChanged` toggles and the spawn-time
/// `UnitEntered` for units that enter view already sighted — mirroring
/// [`body_state_visuals`].
#[allow(clippy::too_many_arguments)]
pub fn option_visuals(
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingEffectStates>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    children_query: Query<&Children>,
    orbits: Query<Entity, With<SightOrbit>>,
) {
    for event in state_changes.read() {
        match registry.get_entity(event.unit_id) {
            Some(entity) => apply_sight_state(
                &mut commands,
                entity,
                event.effect_state,
                &mut meshes,
                &mut materials,
                &children_query,
                &orbits,
            ),
            None => {
                pending.0.insert(event.unit_id, event.effect_state);
            }
        }
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_sight_state(
            &mut commands,
            entity,
            event.effect_state,
            &mut meshes,
            &mut materials,
            &children_query,
            &orbits,
        );
    }

    pending.0.retain(|&unit_id, &mut effect_state| {
        let Some(entity) = registry.get_entity(unit_id) else {
            return true;
        };
        apply_sight_state(
            &mut commands,
            entity,
            effect_state,
            &mut meshes,
            &mut materials,
            &children_query,
            &orbits,
        );
        false
    });
}

fn apply_sight_state(
    commands: &mut Commands,
    entity: Entity,
    effect_state: u32,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    children_query: &Query<&Children>,
    orbits: &Query<Entity, With<SightOrbit>>,
) {
    let sight_on = effect_state & OPTION_SIGHT != 0;
    let existing_child = children_query
        .get(entity)
        .ok()
        .and_then(|children| children.iter().find(|child| orbits.contains(*child)));

    match (sight_on, existing_child) {
        (true, None) => {
            let mesh = meshes.add(Rectangle::new(0.6, 0.6));
            let material = materials.add(StandardMaterial {
                base_color: SIGHT_ORBIT_COLOR,
                unlit: true,
                cull_mode: None,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            commands.spawn((
                SightOrbit,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(SIGHT_ORBIT_RADIUS, SIGHT_ORBIT_LIFT, 0.0),
                Visibility::default(),
                ChildOf(entity),
            ));
        }
        (false, Some(child)) => {
            commands.entity(child).despawn();
        }
        _ => {}
    }
}

/// Rotates every [`SightOrbit`] child's local offset around its parent unit
/// each frame, a fixed radius/period circle lifted above the ground.
pub fn orbit_sight_visuals(time: Res<Time>, mut orbits: Query<&mut Transform, With<SightOrbit>>) {
    let angle = time.elapsed_secs() * (TAU / SIGHT_ORBIT_PERIOD_SECS);
    let (sin, cos) = angle.sin_cos();
    for mut transform in &mut orbits {
        transform.translation = Vec3::new(
            cos * SIGHT_ORBIT_RADIUS,
            SIGHT_ORBIT_LIFT,
            sin * SIGHT_ORBIT_RADIUS,
        );
    }
}

/// Marks a status-driven aura effect spawned as a child of the unit it
/// belongs to, tagged with the EFST that owns it so [`efst_auras`] can find
/// and despawn the right child on `on=false`.
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusAura {
    pub efst: u32,
}

/// Attaches/detaches a `StatusEffectCatalog`-mapped repeating STR as a child
/// of the unit named by `StatusEffectChanged.unit_id`. Most EFSTs have no
/// catalog entry (only auras like Energy Coat do), so a catalog miss is a
/// silent no-op, not a warning — the sparse catalog is by design.
///
/// A `unit_id` that has not registered yet on `on=true` is warned and
/// skipped rather than buffered: unlike Sight (a single bit reapplied from
/// `UnitEntered` at spawn), an aura can be one of many concurrent EFSTs per
/// unit, so a correct buffer would need a per-efst map — not worth it for a
/// window that only matters if the aura's `StatusEffectChanged` races ahead
/// of the unit's own spawn packet. `on=false` for an unresolved unit is a
/// silent no-op: there is nothing to despawn.
pub fn efst_auras(
    mut events: MessageReader<StatusEffectChanged>,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<StatusEffectCatalog>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    children_query: Query<&Children>,
    auras: Query<&StatusAura>,
) {
    for event in events.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            if event.on {
                warn!(
                    "StatusEffectChanged(efst {}, on) for unresolved unit {}; aura skipped",
                    event.efst, event.unit_id
                );
            }
            continue;
        };

        let existing_child = children_query.get(entity).ok().and_then(|children| {
            children
                .iter()
                .find(|child| auras.get(*child).is_ok_and(|aura| aura.efst == event.efst))
        });

        if !event.on {
            if let Some(child) = existing_child {
                commands.entity(child).despawn();
            }
            continue;
        }

        if existing_child.is_some() {
            continue;
        }

        let Some(descriptor) = catalog.as_deref().and_then(|c| c.get(event.efst)) else {
            continue;
        };
        let Some(handle) = load_effect(&asset_server, descriptor) else {
            continue;
        };
        let effect = spawn_effect(
            &mut commands,
            handle,
            EffectAnchor::Position(Vec3::ZERO),
            descriptor.repeating,
            descriptor_tint(descriptor),
            None,
        );
        commands
            .entity(effect)
            .insert((StatusAura { efst: event.efst }, ChildOf(entity)));
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

    fn sight_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .add_message::<UnitStateChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingEffectStates>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_systems(Update, option_visuals);
        app
    }

    fn emit_effect_state(app: &mut App, unit_id: u32, effect_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id,
                body_state: 0,
                health_state: 0,
                effect_state,
                virtue: 0,
            });
        app.update();
    }

    fn orbit_child(app: &mut App, unit: Entity) -> Option<Entity> {
        app.world().get::<Children>(unit).and_then(|children| {
            children
                .iter()
                .find(|child| app.world().get::<SightOrbit>(*child).is_some())
        })
    }

    #[test]
    fn sight_bit_spawns_orbit_child() {
        let mut app = sight_app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);

        assert!(
            orbit_child(&mut app, unit).is_some(),
            "sight bit spawns an orbit child"
        );
    }

    #[test]
    fn clearing_sight_bit_despawns_orbit_child() {
        let mut app = sight_app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);
        assert!(orbit_child(&mut app, unit).is_some());

        emit_effect_state(&mut app, 7, 0);
        assert!(
            orbit_child(&mut app, unit).is_none(),
            "clearing the bit despawns the orbit child"
        );
    }

    #[test]
    fn repeated_sight_bit_does_not_stack_children() {
        let mut app = sight_app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);
        emit_effect_state(&mut app, 7, OPTION_SIGHT | 2); // sight stays on, another bit toggles

        let count = app
            .world()
            .get::<Children>(unit)
            .map(|children| {
                children
                    .iter()
                    .filter(|child| app.world().get::<SightOrbit>(*child).is_some())
                    .count()
            })
            .unwrap_or(0);
        assert_eq!(count, 1, "re-applying the bit does not spawn a duplicate");
    }

    #[test]
    fn sight_state_before_registration_applies_after_it() {
        let mut app = sight_app();

        emit_effect_state(&mut app, 7, OPTION_SIGHT);

        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);
        app.update();

        assert!(
            orbit_child(&mut app, unit).is_some(),
            "buffered sight state applies once the unit resolves"
        );
    }

    fn seeded_status_catalog() -> StatusEffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset =
            ron::from_str::<crate::infrastructure::effect::EffectDataAsset>(ron).expect("seed RON");
        StatusEffectCatalog::from_status_effect_data(asset.0.statuses)
    }

    const EFST_ENERGYCOAT: u32 = 31; // aesir Efst.id(:energycoat).

    fn aura_app(catalog: StatusEffectCatalog) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<crate::infrastructure::effect::LoadedEffectAsset>()
            .add_message::<StatusEffectChanged>()
            .init_resource::<EntityRegistry>()
            .insert_resource(catalog)
            .add_systems(Update, efst_auras);
        app
    }

    fn emit_status_effect(app: &mut App, unit_id: u32, efst: u32, on: bool) {
        app.world_mut()
            .resource_mut::<Messages<StatusEffectChanged>>()
            .write(StatusEffectChanged {
                unit_id,
                efst,
                on,
                total_ms: 0,
                remain_ms: 0,
            });
        app.update();
    }

    fn aura_children(app: &mut App, unit: Entity) -> Vec<u32> {
        let world = app.world();
        world
            .get::<Children>(unit)
            .map(|children| {
                children
                    .iter()
                    .filter_map(|child| world.get::<StatusAura>(child).map(|aura| aura.efst))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn efst_on_with_catalog_hit_attaches_aura_child() {
        let mut app = aura_app(seeded_status_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);

        assert_eq!(aura_children(&mut app, unit), vec![EFST_ENERGYCOAT]);
    }

    #[test]
    fn efst_off_detaches_aura_child() {
        let mut app = aura_app(seeded_status_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);
        assert_eq!(aura_children(&mut app, unit), vec![EFST_ENERGYCOAT]);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, false);
        assert!(aura_children(&mut app, unit).is_empty());
    }

    #[test]
    fn efst_on_without_catalog_entry_is_a_noop() {
        let mut app = aura_app(seeded_status_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, 9999, true);

        assert!(aura_children(&mut app, unit).is_empty());
    }

    #[test]
    fn efst_repeated_on_does_not_stack_children() {
        let mut app = aura_app(seeded_status_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);
        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);

        assert_eq!(aura_children(&mut app, unit), vec![EFST_ENERGYCOAT]);
    }

    #[test]
    fn efst_on_for_unresolved_unit_is_warn_and_skip() {
        let mut app = aura_app(seeded_status_catalog());

        // No entity registered for unit_id 7: must not panic, and there is
        // nothing to attach the aura to.
        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);

        let mut auras = app.world_mut().query::<&StatusAura>();
        assert_eq!(auras.iter(app.world()).count(), 0);
    }
}
