//! Owner-only Homunculus controls. The server owns the companion state.

use bevy::{prelude::*, text::EditableText, ui_widgets::Activate};
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_client::{
    core::state::GameState,
    domain::{
        entities::markers::LocalPlayer,
        homunculus::{
            HomunculusFeedbackKind, HomunculusIntent, HomunculusPresence, HomunculusState,
        },
        input::{PlayerAction, TargetingMode, targeting::HomunculusUnitTargetAction, ui_unfocused},
    },
};
use lifthrasir_runtime::actors::ActorVisualRoot;
use net_contract::dto::{
    HomunculusAction as Action, HomunculusError, HomunculusLifecycle, HomunculusSnapshot,
};

mod portrait;
mod scene;
mod skills;
mod tactics;

pub struct HomunculusWindowPlugin;
impl Plugin for HomunculusWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OwnerUi>()
            .add_systems(
                Update,
                (
                    toggle_key.run_if(ui_unfocused),
                    update_window,
                    portrait::update,
                    sync_tabs,
                    skills::update,
                    tactics::update,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), reset)
            .add_observer(on_world_click);
    }
}

#[derive(Component, Default, Clone)]
struct HomunculusWindow;
#[derive(Component, Default, Clone)]
struct StatusText;
#[derive(Component, Default, Clone)]
struct Portrait;
#[derive(Component, Default, Clone)]
struct NameField;
#[derive(Component, Default, Clone)]
struct ConfirmPanel;
#[derive(Component, Default, Clone)]
struct ConfirmText;
#[derive(Component, Default, Clone)]
struct OwnerMenu;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CareButton {
    #[default]
    Close,
    Inspect,
    Follow,
    Standby,
    Move,
    Attack,
    Feed,
    Rest,
    Rename,
    Delete,
    Confirm,
    Cancel,
}

#[derive(Clone, Copy)]
struct Confirmation {
    id: u64,
    delete: bool,
}

#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
enum HomunculusTab {
    #[default]
    Status,
    Skills,
    Tactics,
}

fn sync_tabs(ui: Res<OwnerUi>, mut pages: Query<(&HomunculusTab, &mut Node)>) {
    for (page, mut node) in &mut pages {
        let display = if *page == ui.tab {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

#[derive(Resource, Default)]
struct OwnerUi {
    tab: HomunculusTab,
    skill_ranks: std::collections::HashMap<u32, u32>,
    tactics: tactics::Editor,
    open: bool,
    was_open: bool,
    confirmation: Option<Confirmation>,
}

pub fn spawn_window(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(ChildOf(parent));
}

fn toggle_key(
    actions: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut ui: ResMut<OwnerUi>,
) {
    if actions
        .single()
        .is_ok_and(|keys| keys.just_pressed(&PlayerAction::Homunculus))
    {
        ui.open = !ui.open;
    }
}

fn reset(mut ui: ResMut<OwnerUi>, mut out: MessageWriter<HomunculusIntent>) {
    *ui = OwnerUi::default();
    out.write(HomunculusIntent::WindowVisibility(false));
}

fn owns_pick(state: &HomunculusState, picked: Entity, visual: Option<&ActorVisualRoot>) -> bool {
    state
        .controlled_actor()
        .is_some_and(|actor| actor == visual.map_or(picked, |part| part.0))
}

fn on_world_click(
    mut click: On<PointerClick>,
    game: Res<State<GameState>>,
    state: Res<HomunculusState>,
    visuals: Query<&ActorVisualRoot>,
    menus: Query<Entity, With<OwnerMenu>>,
    mut commands: Commands,
) {
    if *game.get() != GameState::InGame {
        return;
    }
    if !menus.contains(click.entity) {
        for menu in &menus {
            commands.entity(menu).try_despawn();
        }
    }
    if click.button != PointerButton::Secondary
        || !owns_pick(&state, click.entity, visuals.get(click.entity).ok())
    {
        return;
    }
    click.propagate(false);
    commands
        .spawn_scene(scene::context_menu(click.pointer.position))
        .insert(DespawnOnExit(GameState::InGame));
}

fn confirm(confirmation: Confirmation, current: &HomunculusSnapshot) -> Option<Action> {
    (confirmation.id == current.durable_id).then_some(if confirmation.delete {
        Action::Delete { confirmed: true }
    } else {
        Action::Feed
    })
}

fn activate(
    event: On<Activate>,
    buttons: Query<&CareButton>,
    names: Query<&EditableText, With<NameField>>,
    state: Res<HomunculusState>,
    mut ui: ResMut<OwnerUi>,
    mut targeting: ResMut<TargetingMode>,
    mut out: MessageWriter<HomunculusIntent>,
) {
    let Ok(&button) = buttons.get(event.entity) else {
        return;
    };
    match button {
        CareButton::Close => {
            ui.open = false;
            return;
        }
        CareButton::Cancel => {
            ui.confirmation = None;
            return;
        }
        CareButton::Inspect => {
            out.write(HomunculusIntent::Inspect);
            return;
        }
        _ => {}
    }
    if !ui.open || !state.can_mutate() {
        return;
    }
    let Some(current) = state.snapshot() else {
        return;
    };
    let action = match button {
        CareButton::Move | CareButton::Attack => {
            let (Some(gid), Some(actor)) = (state.controlled_gid(), state.controlled_actor())
            else {
                return;
            };
            let durable_id = current.durable_id;
            *targeting = if button == CareButton::Move {
                TargetingMode::HomunculusMove {
                    durable_id,
                    gid,
                    actor,
                }
            } else {
                TargetingMode::HomunculusUnit {
                    durable_id,
                    gid,
                    actor,
                    action: HomunculusUnitTargetAction::Attack,
                }
            };
            return;
        }
        CareButton::Follow => Action::Follow,
        CareButton::Standby => Action::Standby,
        CareButton::Rest => Action::Rest,
        CareButton::Feed if current.hunger <= 75 => Action::Feed,
        CareButton::Feed | CareButton::Delete => {
            if button == CareButton::Feed && state.controlled_gid().is_none() {
                return;
            }
            ui.confirmation = Some(Confirmation {
                id: current.durable_id,
                delete: button == CareButton::Delete,
            });
            return;
        }
        CareButton::Rename if current.rename_eligible => {
            let Ok(text) = names.single() else {
                return;
            };
            let name = text.value().to_string();
            let name = name.trim();
            if name.is_empty() {
                return;
            }
            Action::Rename {
                name: name.to_owned(),
            }
        }
        CareButton::Confirm => {
            let Some(action) = ui
                .confirmation
                .take()
                .and_then(|pending| confirm(pending, current))
            else {
                return;
            };
            action
        }
        _ => return,
    };
    out.write(HomunculusIntent::Act {
        durable_id: current.durable_id,
        action,
    });
}

#[derive(bevy::ecs::system::SystemParam)]
struct Nodes<'w, 's> {
    windows: Query<'w, 's, &'static mut Node, (With<HomunculusWindow>, Without<ConfirmPanel>)>,
    confirm: Query<'w, 's, &'static mut Node, (With<ConfirmPanel>, Without<HomunculusWindow>)>,
    status: Query<'w, 's, &'static mut Text, (With<StatusText>, Without<ConfirmText>)>,
    prompt: Query<'w, 's, &'static mut Text, (With<ConfirmText>, Without<StatusText>)>,
    menus: Query<'w, 's, Entity, With<OwnerMenu>>,
}

fn update_window(
    mut ui: ResMut<OwnerUi>,
    state: Res<HomunculusState>,
    time: Res<Time<Real>>,
    items: Option<Res<lifthrasir_assets::catalogs::ItemDb>>,
    mut nodes: Nodes,
    mut commands: Commands,
    mut out: MessageWriter<HomunculusIntent>,
) {
    let Ok(mut window) = nodes.windows.single_mut() else {
        return;
    };
    if ui.open != ui.was_open {
        window.display = if ui.open {
            Display::Flex
        } else {
            Display::None
        };
        out.write(HomunculusIntent::WindowVisibility(ui.open));
        ui.was_open = ui.open;
        ui.confirmation = None;
        for menu in &nodes.menus {
            commands.entity(menu).try_despawn();
        }
    }
    if !ui.open {
        return;
    }
    if ui.confirmation.is_some_and(|c| {
        !state.can_mutate() || !state.snapshot().is_some_and(|s| s.durable_id == c.id)
    }) {
        ui.confirmation = None;
    }
    if let Ok(mut text) = nodes.status.single_mut() {
        let label = status(&state, time.elapsed(), items.as_deref());
        if text.0 != label {
            text.0 = label;
        }
    }
    if let Ok(mut panel) = nodes.confirm.single_mut() {
        let prompt = ui.confirmation.zip(state.snapshot()).map(|(p, s)| if p.delete { format!("Permanently delete {}?", s.name) } else { format!("Overfeeding {} lowers intimacy and can permanently lose your companion. Feed anyway?", s.name) });
        panel.display = if prompt.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if let (Some(prompt), Ok(mut text)) = (prompt, nodes.prompt.single_mut())
            && text.0 != prompt
        {
            text.0 = prompt;
        }
    }
}

fn status(
    state: &HomunculusState,
    now: std::time::Duration,
    items: Option<&lifthrasir_assets::catalogs::ItemDb>,
) -> String {
    let mut label = match state.presence() {
        HomunculusPresence::Unknown => "Loading companion…".into(),
        HomunculusPresence::Absent => "No companion. Use Call Homunculus to create one.".into(),
        HomunculusPresence::Present(s) => {
            let lifecycle = match s.lifecycle {
                HomunculusLifecycle::Active => "Active",
                HomunculusLifecycle::Rested => "Rested",
                HomunculusLifecycle::Dead => "Dead",
                _ => "Unknown",
            };
            let food = items.and_then(|db| db.get(s.food_item_id)).map_or_else(
                || format!("#{}", s.food_item_id),
                |item| item.identified_name.clone(),
            );
            let mut text = format!(
                "{}  ·  Lv. {}  ·  {lifecycle}\n{:?}  ·  {}s remaining\nHP {} / {}    SP {} / {}\nEXP {} / {}\nHunger {}%    Intimacy {:.2} ({:?})\nFood: {food}",
                s.name,
                s.level,
                s.activity,
                state.remaining(s.active_remaining_ms, now) / 1000,
                s.hp,
                s.max_hp,
                s.sp,
                s.max_sp,
                s.exp,
                s.next_exp,
                s.hunger,
                f64::from(s.intimacy_hundredths) / 100.0,
                s.intimacy_grade
            );
            if let Some(stats) = &s.stats {
                use std::fmt::Write;
                let _ = write!(
                    text,
                    "\nSTR {}  AGI {}  VIT {}  INT {}  DEX {}  LUK {}\nATK {}  MATK {}  DEF {}  MDEF {}\nHIT {}  FLEE {}  CRIT {}  ASPD {}",
                    stats.str,
                    stats.agi,
                    stats.vit,
                    stats.int,
                    stats.dex,
                    stats.luk,
                    stats.atk,
                    stats.matk,
                    stats.def,
                    stats.mdef,
                    stats.hit,
                    stats.flee,
                    stats.critical,
                    stats.aspd
                );
            }
            text
        }
    };
    use std::fmt::Write;
    match state.last_feedback() {
        Some(HomunculusFeedbackKind::Rejected(error)) => {
            let _ = write!(label, "\n{}", error_text(*error));
        }
        Some(HomunculusFeedbackKind::SendFailed(reason)) => {
            let _ = write!(label, "\n{reason}");
        }
        Some(HomunculusFeedbackKind::TimedOut) => {
            label.push_str("\nNo reply received. Refresh to try again.")
        }
        Some(HomunculusFeedbackKind::Removed) => {
            label.push_str("\nYour companion was permanently removed.")
        }
        _ if state.pending_mutation().is_some() => label.push_str("\nWaiting for the server…"),
        _ => {}
    }
    label
}

fn error_text(error: HomunculusError) -> &'static str {
    use HomunculusError::*;
    match error {
        NoCompanion => "You do not have a companion.",
        InvalidLifecycle => "Your companion cannot do that in its current state.",
        InvalidPosition => "Choose a walkable position.",
        InvalidTarget => "That target is not available.",
        OutOfRange => "That target is out of range.",
        SkillNotLearned => "Your companion has not learned that skill.",
        InvalidSkillRank => "That skill rank is not available.",
        OnCooldown => "That skill is on cooldown.",
        InsufficientSp => "Not enough SP.",
        MissingItem => "The required item is missing.",
        HpGate => "Your companion does not have enough HP.",
        RenameNotAllowed => "This companion can no longer be renamed.",
        InvalidName => "That name is invalid.",
        ConfirmationRequired => "Confirm before deleting your companion.",
        InvalidAiConfig => "The tactics configuration was rejected.",
        InsufficientSkillPoints => "Not enough skill points.",
        PrerequisitesNotMet => "Skill prerequisites are not met.",
        Busy => "Your companion is busy.",
        MalformedCommand | WrongChannel | None | Unknown(_) => "The server rejected that command.",
    }
}

#[cfg(test)]
mod tests;
