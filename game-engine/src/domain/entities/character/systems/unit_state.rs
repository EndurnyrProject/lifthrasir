use crate::domain::entities::character::components::UnitState;
use crate::domain::entities::registry::EntityRegistry;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::events::UnitStateChanged;

// option (effect_state) bitmask ids. Values are the rAthena `e_option`
// constants aesir sends (Aesir.ZoneServer.Mmo.Option).
const OPTION_HIDE: u32 = 2;
const OPTION_CLOAK: u32 = 4;

/// Pushcart mount bits. A merchant with any cart tier sets one of these in
/// `effect_state`; we render a single sprite regardless of tier, so
/// `CART_MASK` collapses all three into one "mounted" test.
const OPTION_CART1: u32 = 0x08;
const OPTION_CART2: u32 = 0x80;
const OPTION_CART3: u32 = 0x100;
pub(crate) const CART_MASK: u32 = OPTION_CART1 | OPTION_CART2 | OPTION_CART3;

impl UnitState {
    /// Whether any pushcart tier bit is set in `effect_state`. The UI reads this
    /// off the local player to decide between the mount prompt and the mounted
    /// body; it is the same test the cart render layer uses.
    pub fn is_cart_mounted(&self) -> bool {
        self.effect_state & CART_MASK != 0
    }
}

/// Consumes the legacy `UnitStateChange` channel: stores all four state fields
/// on [`UnitState`] and renders the cheap high-value subset.
///
/// ponytail: only hide/cloak visibility (option) renders. Deliberately deferred
/// and stored-only:
/// - stone/freeze/stun/sleep body poses (opt1): no dedicated `AnimationState`
///   variants exist for them, and mapping onto `Hit` collides with the combat
///   HitStun revert (`update_hit_stun` forces unmarked `Hit` back to `Idle`
///   after ~0.3s), so the pose would flicker then vanish while still active.
///   Doing it right needs real stun/freeze/sleep animation states.
/// - poison/curse/silence tint (health_state/opt2).
/// - mount/orc-head and the other option bits. (The cart bits are consumed by
///   `apply_cart_mount` in the sprite-rendering domain.)
/// - virtue (opt3).
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update
)]
pub fn apply_unit_state(
    mut events: MessageReader<UnitStateChanged>,
    registry: Res<EntityRegistry>,
    mut commands: Commands,
    mut visibilities: Query<&mut Visibility>,
) {
    for event in events.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            continue;
        };

        commands.entity(entity).insert(UnitState {
            body_state: event.body_state,
            health_state: event.health_state,
            effect_state: event.effect_state,
            virtue: event.virtue,
        });

        apply_hide_cloak(entity, event.effect_state, &mut visibilities);
    }
}

fn apply_hide_cloak(entity: Entity, effect_state: u32, visibilities: &mut Query<&mut Visibility>) {
    let Ok(mut visibility) = visibilities.get_mut(entity) else {
        return;
    };

    let hidden = effect_state & (OPTION_HIDE | OPTION_CLOAK) != 0;
    let target = if hidden {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    if *visibility != target {
        *visibility = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // opt1 body_state, kept only to prove the field is stored (not rendered).
    const OPT1_SLEEP: u32 = 4;

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<UnitStateChanged>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, apply_unit_state);
        app
    }

    fn register(app: &mut App, gid: u32, entity: Entity) {
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
    }

    fn emit(app: &mut App, event: UnitStateChanged) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(event);
        app.update();
    }

    #[test]
    fn stores_all_four_fields_on_resolved_entity() {
        let mut app = app();
        let entity = app.world_mut().spawn_empty().id();
        register(&mut app, 7, entity);

        emit(
            &mut app,
            UnitStateChanged {
                unit_id: 7,
                body_state: OPT1_SLEEP,
                health_state: 2,
                effect_state: OPTION_HIDE,
                virtue: 3,
            },
        );

        assert_eq!(
            app.world().entity(entity).get::<UnitState>(),
            Some(&UnitState {
                body_state: OPT1_SLEEP,
                health_state: 2,
                effect_state: OPTION_HIDE,
                virtue: 3,
            })
        );
    }

    #[test]
    fn hide_cloak_bit_toggles_visibility() {
        let mut app = app();
        let entity = app.world_mut().spawn(Visibility::Inherited).id();
        register(&mut app, 7, entity);

        emit(
            &mut app,
            UnitStateChanged {
                unit_id: 7,
                body_state: 0,
                health_state: 0,
                effect_state: OPTION_CLOAK,
                virtue: 0,
            },
        );
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );

        emit(
            &mut app,
            UnitStateChanged {
                unit_id: 7,
                body_state: 0,
                health_state: 0,
                effect_state: 0,
                virtue: 0,
            },
        );
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn is_cart_mounted_reflects_cart_bits() {
        assert!(!UnitState::default().is_cart_mounted());
        for bit in [OPTION_CART1, OPTION_CART2, OPTION_CART3] {
            assert!(UnitState {
                effect_state: bit,
                ..Default::default()
            }
            .is_cart_mounted());
        }
        assert!(!UnitState {
            effect_state: OPTION_HIDE,
            ..Default::default()
        }
        .is_cart_mounted());
    }

    #[test]
    fn unknown_unit_id_is_a_no_op() {
        let mut app = app();
        let entity = app.world_mut().spawn_empty().id();
        register(&mut app, 7, entity);

        emit(
            &mut app,
            UnitStateChanged {
                unit_id: 999,
                body_state: OPT1_SLEEP,
                health_state: 1,
                effect_state: OPTION_HIDE,
                virtue: 1,
            },
        );

        assert!(app.world().entity(entity).get::<UnitState>().is_none());
    }
}
