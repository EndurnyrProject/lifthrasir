use super::observers::handle_status_effect_state_changes;
use super::status_effects::StatusEffects;
use crate::domain::entities::registry::EntityRegistry;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::events::StatusEffectChanged;

/// rAthena EFST id for Play Dead (SC_TRICKDEAD). Aesir carries no opt field for
/// it, so this icon toggle is the only signal that drives the dead pose.
const EFST_TRICKDEAD: u32 = 29;

/// Folds inbound EFST toggles into each unit's [`StatusEffects`] so the animation
/// observer can react. Only EFSTs that map onto a tracked field touch the
/// component, which keeps change detection from firing on unrelated icons.
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(before = handle_status_effect_state_changes)
)]
pub fn apply_status_effect_changes(
    mut events: MessageReader<StatusEffectChanged>,
    registry: Res<EntityRegistry>,
    mut characters: Query<&mut StatusEffects>,
) {
    for event in events.read() {
        if event.efst != EFST_TRICKDEAD {
            continue;
        }

        let Some(entity) = registry.get_entity(event.unit_id) else {
            continue;
        };

        let Ok(mut status) = characters.get_mut(entity) else {
            continue;
        };

        status.dead = event.on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<StatusEffectChanged>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, apply_status_effect_changes);
        app
    }

    fn emit(app: &mut App, unit_id: u32, efst: u32, on: bool) {
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

    #[test]
    fn trickdead_toggles_dead_field() {
        let mut app = app();
        let entity = app.world_mut().spawn(StatusEffects::default()).id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(7, entity);

        emit(&mut app, 7, EFST_TRICKDEAD, true);
        assert!(
            app.world()
                .entity(entity)
                .get::<StatusEffects>()
                .unwrap()
                .dead
        );

        emit(&mut app, 7, EFST_TRICKDEAD, false);
        assert!(
            !app.world()
                .entity(entity)
                .get::<StatusEffects>()
                .unwrap()
                .dead
        );
    }

    #[test]
    fn unrelated_efst_leaves_status_unchanged() {
        let mut app = app();
        let entity = app.world_mut().spawn(StatusEffects::default()).id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(7, entity);

        emit(&mut app, 7, 1, true);
        assert!(
            !app.world()
                .entity(entity)
                .get::<StatusEffects>()
                .unwrap()
                .dead
        );
    }
}
