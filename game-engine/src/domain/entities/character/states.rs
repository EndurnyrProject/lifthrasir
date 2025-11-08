use crate::domain::entities::character::components::visual::ActionType;
use crate::domain::entities::sprite_rendering::{
    EffectType, SpriteAnimationChangeEvent, StatusEffectVisualEvent,
};
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_event, auto_add_system};
use seldom_state::prelude::*;

// Animation states for character actions
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Component, Reflect)]
#[component(storage = "SparseSet")]
#[derive(Default)]
pub enum AnimationState {
    #[default]
    Idle,
    Walking,
    Sitting,
    Attacking,
    Casting,
    Dead,
    Hit,
    Pickup,
    Special,
}

// Gameplay states that affect character behavior
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Component, Reflect)]
#[component(storage = "SparseSet")]
#[derive(Default)]
pub enum GameplayState {
    #[default]
    Normal,
    Stunned,
    Frozen,
    Petrified,
    Sleeping,
    Poisoned,
    Bleeding,
    Hiding,
    Cloaking,
    Dead,
}

// Context states for different game modes
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Component, Reflect)]
#[component(storage = "SparseSet")]
#[derive(Default)]
pub enum ContextState {
    #[default]
    CharacterSelection,
    InGame,
    InBattle,
    InTrade,
    InShop,
    InChat,
    InMenu,
}

// Events for state changes
#[derive(Message, Debug)]
#[auto_add_event(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct CharacterStateChangeEvent {
    pub entity: Entity,
    pub animation_state: Option<AnimationState>,
    pub gameplay_state: Option<GameplayState>,
    pub context_state: Option<ContextState>,
    pub trigger: StateChangeTrigger,
}

#[derive(Debug, Clone)]
pub enum StateChangeTrigger {
    Movement,
    Combat,
    UserInput,
    GameEvent,
    NetworkEvent,
    Animation,
}

// State machine triggers for animation transitions
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct StartWalking;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct StopWalking;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct StartAttacking;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct FinishAttack;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct StartCasting;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct FinishCasting;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct TakeDamage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct Die;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct Resurrect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct Sit;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct Stand;

// State machine triggers for gameplay state transitions
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct GetStunned;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct RecoverFromStun;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct GetPoisoned;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct RecoverFromPoison;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct StartHiding;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct StopHiding;

// State machine triggers for context transitions
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct EnterGame;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct ExitGame;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct EnterBattle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct ExitBattle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct OpenTrade;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect, Component)]
pub struct CloseTrade;

// Default states

// Mapping from AnimationState to ActionType for sprite animations
impl From<AnimationState> for ActionType {
    fn from(state: AnimationState) -> Self {
        match state {
            AnimationState::Idle => ActionType::Idle,
            AnimationState::Walking => ActionType::Walk,
            AnimationState::Sitting => ActionType::Sit,
            AnimationState::Attacking => ActionType::Attack,
            AnimationState::Casting => ActionType::Cast,
            AnimationState::Dead => ActionType::Dead,
            AnimationState::Hit => ActionType::Hit,
            AnimationState::Pickup => ActionType::Special,
            AnimationState::Special => ActionType::Special,
        }
    }
}

// State machine setup function
// Note: Systems and CharacterStateChangeEvent are now auto-registered via bevy_auto_plugin
pub fn setup_character_state_machines(app: &mut App) {
    app.add_plugins(StateMachinePlugin::default());
}

// Trigger insertion systems for automatic state transitions
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update
)]
pub fn insert_animation_triggers_from_gameplay_changes(
    mut commands: Commands,
    characters: CharactersWithChangedGameplayState,
) {
    for (entity, gameplay_state, animation_state) in characters.iter() {
        match (gameplay_state, animation_state) {
            (GameplayState::Dead, animation_state) if *animation_state != AnimationState::Dead => {
                commands.entity(entity).insert(Die);
            }
            (
                GameplayState::Stunned | GameplayState::Frozen | GameplayState::Petrified,
                animation_state,
            ) if *animation_state != AnimationState::Hit => {
                commands.entity(entity).insert(TakeDamage);
            }
            (GameplayState::Sleeping, animation_state)
                if *animation_state != AnimationState::Sitting =>
            {
                commands.entity(entity).insert(Sit);
            }
            (GameplayState::Normal, animation_state)
                if *animation_state != AnimationState::Idle =>
            {
                // Only transition to idle if not already there
                commands.entity(entity).insert(StopWalking);
            }
            _ => {} // No state mismatch, no trigger needed
        }
    }
}

// State change observer systems - these connect state machine to sprite visuals

type ChangedAnimationsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static AnimationState,
        Option<&'static crate::domain::entities::character::components::visual::CharacterDirection>,
    ),
    (
        Changed<AnimationState>,
        With<seldom_state::prelude::StateMachine>,
    ),
>;

/// Observes AnimationState changes and emits sprite animation events
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(after = insert_animation_triggers_from_gameplay_changes)
)]
pub fn observe_animation_state_changes(
    mut animation_events: MessageWriter<SpriteAnimationChangeEvent>,
    changed_animations: ChangedAnimationsQuery,
) {
    for (entity, animation_state, direction) in changed_animations.iter() {
        let action_type = (*animation_state).into();

        debug!(
            "🎭 AnimationState CHANGED for {:?}: {:?} -> ActionType::{:?}",
            entity, animation_state, action_type
        );

        animation_events.write(SpriteAnimationChangeEvent {
            character_entity: entity,
            action_type,
        });

        if let Some(dir) = direction {
            debug!("   └─ Direction: {:?}", dir.facing);
        }
    }
}

/// Observes GameplayState changes and emits status effect visual events
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(after = observe_animation_state_changes)
)]
pub fn observe_gameplay_state_changes(
    mut effect_events: MessageWriter<StatusEffectVisualEvent>,
    changed_gameplay: CharactersWithChangedGameplay,
) {
    for (entity, gameplay_state) in changed_gameplay.iter() {
        info!(
            "Gameplay state changed for {:?}: {:?}",
            entity, gameplay_state
        );

        match gameplay_state {
            GameplayState::Poisoned => {
                effect_events.write(StatusEffectVisualEvent {
                    character: entity,
                    effect_type: EffectType::Poison,
                    add: true,
                });
            }
            GameplayState::Stunned => {
                effect_events.write(StatusEffectVisualEvent {
                    character: entity,
                    effect_type: EffectType::Stun,
                    add: true,
                });
            }
            GameplayState::Frozen => {
                effect_events.write(StatusEffectVisualEvent {
                    character: entity,
                    effect_type: EffectType::Freeze,
                    add: true,
                });
            }
            GameplayState::Petrified => {
                effect_events.write(StatusEffectVisualEvent {
                    character: entity,
                    effect_type: EffectType::Stone,
                    add: true,
                });
            }
            GameplayState::Sleeping => {
                effect_events.write(StatusEffectVisualEvent {
                    character: entity,
                    effect_type: EffectType::Sleep,
                    add: true,
                });
            }
            GameplayState::Normal => {
                // Remove all temporary status effects when returning to normal
                for effect_type in [
                    EffectType::Poison,
                    EffectType::Stun,
                    EffectType::Freeze,
                    EffectType::Stone,
                    EffectType::Sleep,
                ] {
                    effect_events.write(StatusEffectVisualEvent {
                        character: entity,
                        effect_type,
                        add: false,
                    });
                }
            }
            _ => {} // Other states don't have specific visual effects
        }
    }
}

/// Type alias for characters with changed gameplay state
type CharactersWithChangedGameplayState<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static GameplayState, &'static AnimationState),
    (With<StateMachine>, Changed<GameplayState>),
>;

/// Type alias for characters with changed gameplay state (simple version)
type CharactersWithChangedGameplay<'w, 's> =
    Query<'w, 's, (Entity, &'static GameplayState), (Changed<GameplayState>, With<StateMachine>)>;

// Create a unified state machine that handles all character state types
pub fn create_animation_state_machine() -> StateMachine {
    StateMachine::default()
        // ANIMATION STATE TRANSITIONS
        // Basic movement transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&StartWalking>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Walking))
            },
            AnimationState::Walking,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&StopWalking>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Combat transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&StartAttacking>| triggers.get(entity).is_ok(),
            AnimationState::Attacking,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&FinishAttack>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Casting transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&StartCasting>| triggers.get(entity).is_ok(),
            AnimationState::Casting,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&FinishCasting>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Sitting transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&Sit>| triggers.get(entity).is_ok(),
            AnimationState::Sitting,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&Stand>, states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Damage and death transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&TakeDamage>| triggers.get(entity).is_ok(),
            AnimationState::Hit,
        )
        // Death transitions (from any animation state)
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&Die>| triggers.get(entity).is_ok(),
            AnimationState::Dead,
        )
        // Resurrection
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&Resurrect>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Enable transition logging in debug mode
        .set_trans_logging(cfg!(debug_assertions))
}

pub fn create_unified_character_state_machine() -> StateMachine {
    StateMachine::default()
        // ANIMATION STATE TRANSITIONS
        // Basic movement transitions with guards to prevent redundant state changes
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&StartWalking>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Walking))
            },
            AnimationState::Walking,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&StopWalking>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Combat transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&StartAttacking>| triggers.get(entity).is_ok(),
            AnimationState::Attacking,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&FinishAttack>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Casting transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&StartCasting>| triggers.get(entity).is_ok(),
            AnimationState::Casting,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&FinishCasting>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Sitting transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&Sit>| triggers.get(entity).is_ok(),
            AnimationState::Sitting,
        )
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&Stand>, states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // Damage and death transitions
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&TakeDamage>| triggers.get(entity).is_ok(),
            AnimationState::Hit,
        )
        // Death transitions (from any animation state)
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>, triggers: Query<&Die>| triggers.get(entity).is_ok(),
            AnimationState::Dead,
        )
        // Resurrection
        .trans::<AnimationState, _>(
            |In(entity): In<Entity>,
             triggers: Query<&Resurrect>,
             states: Query<&AnimationState>| {
                triggers.get(entity).is_ok()
                    && !matches!(states.get(entity), Ok(AnimationState::Idle))
            },
            AnimationState::Idle,
        )
        // GAMEPLAY STATE TRANSITIONS
        // Status effect transitions
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&GetStunned>| triggers.get(entity).is_ok(),
            GameplayState::Stunned,
        )
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&RecoverFromStun>| {
                triggers.get(entity).is_ok()
            },
            GameplayState::Normal,
        )
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&GetPoisoned>| triggers.get(entity).is_ok(),
            GameplayState::Poisoned,
        )
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&RecoverFromPoison>| {
                triggers.get(entity).is_ok()
            },
            GameplayState::Normal,
        )
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&StartHiding>| triggers.get(entity).is_ok(),
            GameplayState::Hiding,
        )
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&StopHiding>| triggers.get(entity).is_ok(),
            GameplayState::Normal,
        )
        // Death and resurrection (from any gameplay state)
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&Die>| triggers.get(entity).is_ok(),
            GameplayState::Dead,
        )
        .trans::<GameplayState, _>(
            |In(entity): In<Entity>, triggers: Query<&Resurrect>| triggers.get(entity).is_ok(),
            GameplayState::Normal,
        )
        // CONTEXT STATE TRANSITIONS
        // Main game flow transitions
        .trans::<ContextState, _>(
            |In(entity): In<Entity>, triggers: Query<&EnterGame>| triggers.get(entity).is_ok(),
            ContextState::InGame,
        )
        .trans::<ContextState, _>(
            |In(entity): In<Entity>, triggers: Query<&ExitGame>| triggers.get(entity).is_ok(),
            ContextState::CharacterSelection,
        )
        // Battle context
        .trans::<ContextState, _>(
            |In(entity): In<Entity>, triggers: Query<&EnterBattle>| triggers.get(entity).is_ok(),
            ContextState::InBattle,
        )
        .trans::<ContextState, _>(
            |In(entity): In<Entity>, triggers: Query<&ExitBattle>| triggers.get(entity).is_ok(),
            ContextState::InGame,
        )
        // Trading context
        .trans::<ContextState, _>(
            |In(entity): In<Entity>, triggers: Query<&OpenTrade>| triggers.get(entity).is_ok(),
            ContextState::InTrade,
        )
        .trans::<ContextState, _>(
            |In(entity): In<Entity>, triggers: Query<&CloseTrade>| triggers.get(entity).is_ok(),
            ContextState::InGame,
        )
        .set_trans_logging(cfg!(debug_assertions))
}
