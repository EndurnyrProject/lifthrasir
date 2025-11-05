use bevy::prelude::*;

/// Type alias for trigger readers in observer systems.
///
/// Use this instead of raw `On<E>` for consistency across the codebase.
/// In Bevy 0.17, `Trigger` was renamed to `On`.
///
/// # Example
/// ```ignore
/// fn my_observer(trigger: TriggerReader<MyEvent>) {
///     let event = trigger.event();
///     let target = trigger.target();
/// }
/// ```
pub type TriggerReader<'w, 't, E> = On<'w, 't, E>;

/// Type alias for entity-targeted triggers.
///
/// This makes it explicit when an observer expects entity-targeted events.
///
/// # Example
/// ```ignore
/// fn entity_observer(trigger: EntityTrigger<MyEvent>) {
///     let entity = trigger.target();
///     commands.entity(entity).despawn();
/// }
/// ```
pub type EntityTrigger<'w, 't, E> = On<'w, 't, E>;

/// Marker trait for events that are entity-targeted.
///
/// Implement this trait for events that should use the `EntityEvent` pattern.
/// Events implementing this trait should have an `entity: Entity` field
/// annotated with `#[event_target]`.
///
/// # Example
/// ```ignore
/// #[derive(EntityEvent)]
/// struct MyEntityEvent {
///     #[event_target]
///     pub entity: Entity,
///     pub data: String,
/// }
///
/// impl EntityTargeted for MyEntityEvent {}
/// ```
pub trait EntityTargeted: Event {}

/// Macro to reduce boilerplate when defining entity-targeted events.
///
/// This macro generates an Event struct with EntityEvent derive and
/// implements the EntityTargeted trait.
///
/// # Example
/// ```ignore
/// entity_event! {
///     /// Documentation for the event
///     pub struct MovementStopped {
///         pub x: u16,
///         pub y: u16,
///         pub reason: StopReason,
///     }
/// }
/// ```
///
/// Expands to:
/// ```ignore
/// #[derive(EntityEvent, Debug, Clone)]
/// pub struct MovementStopped {
///     #[event_target]
///     pub entity: Entity,
///     pub x: u16,
///     pub y: u16,
///     pub reason: StopReason,
/// }
///
/// impl EntityTargeted for MovementStopped {}
/// ```
#[macro_export]
macro_rules! entity_event {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(EntityEvent, Debug, Clone)]
        $vis struct $name {
            #[event_target]
            pub entity: Entity,
            $(
                $(#[$field_meta])*
                $field_vis $field : $field_ty
            ),*
        }

        impl $crate::core::observers::EntityTargeted for $name {}
    };
}
