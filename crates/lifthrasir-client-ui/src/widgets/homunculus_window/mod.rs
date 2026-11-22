//! The owner's Homunculus window and HUD plate, after the `Homunculus` and
//! `Homunculus (HUD)` artboards of the Lifthrasir Widgets design.
//!
//! The server owns the companion: this module projects [`HomunculusState`] and turns
//! its controls into intents. Each [`Host`] is a region [`rebuild`] refills when its
//! view changes. What changes all the time is written in place instead: the plate's HP
//! and SP, skill cooldowns, and a held slider.
//!
//! NOTE: the artboard also draws a Call Homunculus button on the empty window, calling
//! a rested companion back, a level cap bar, monster names on the never-attack list
//! and a sweep over a cooling skill. The client has no request, cap, name table or
//! cooldown length for those, so they are left out until it does.

use std::collections::HashMap;
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::settings::SaveSettings;
use bevy::text::EditableText;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::ItemDb;
use lifthrasir_assets::catalogs::skill::SkillCatalog;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::homunculus::{
    HomunculusCastIntent, HomunculusFeedbackKind, HomunculusIntent, HomunculusSkillSpec,
    HomunculusState, homunculus_skill_spec,
};
use lifthrasir_client::domain::hotbar::HotbarSlot;
use lifthrasir_client::domain::input::targeting::HomunculusUnitTargetAction;
use lifthrasir_client::domain::input::{PlayerAction, TargetingMode, ui_unfocused};
use lifthrasir_client::domain::inventory::Inventory;
use lifthrasir_client::domain::settings::Keybinds;
use lifthrasir_runtime::actors::ActorVisualRoot;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{
    HomunculusAction as Action, HomunculusAiSkillMode as Mode, HomunculusAiStance as Stance,
    HomunculusError, HomunculusSnapshot,
};

mod hud;
mod portrait;
mod scene;
mod tactics;
mod view;

use crate::screens::shared::{
    SliderOf, SliderPart, SliderRail, keycap_label, light_field_focus, place_slider,
    set_field_text, slider_at, typed_number,
};
use crate::widgets::hotbar::HotbarDrag;
use crate::widgets::status_bar::{Hero, HudSettings};
use tactics::{Dial, Editor, Flag, MobList};

/// Longest name the server takes for a companion.
const NAME_MAX_CHARS: usize = 23;

pub struct HomunculusWindowPlugin;
impl Plugin for HomunculusWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OwnerUi>()
            .add_systems(
                Update,
                (
                    (toggle_key, order_keys).run_if(ui_unfocused),
                    update_window,
                    end_drag,
                    rebuild,
                    (sync_dials, sync_cooldowns, sync_vitals, portrait::update),
                    (
                        light_field_focus::<NameField>,
                        light_field_focus::<MobField>,
                    )
                        .run_if(resource_exists::<InputFocus>),
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
/// The draggable title bar.
#[derive(Component, Default, Clone)]
struct HomunculusTitlebar;
/// The companion's side and the pages, laid out only while there is a companion.
#[derive(Component, Default, Clone)]
struct CompanionLayout;
/// A node [`portrait::update`] draws the companion's sprite on.
#[derive(Component, Default, Clone)]
struct Portrait;
/// The new name, read on Save.
#[derive(Component, Default, Clone)]
struct NameField;
/// The monster class id to add to a list.
#[derive(Component, Default, Clone)]
struct MobField;
#[derive(Component, Default, Clone)]
struct OwnerMenu;
/// The seconds left on a skill's cooldown, written in place.
#[derive(Component, Default, Clone, Copy)]
struct Cooldown(u32);
/// A learned skill's icon, which drags onto the hotbar.
#[derive(Component, Default, Clone, Copy)]
struct SkillIcon(u32);

/// A part of the HUD plate written in place.
#[derive(Component, Default, Clone, Copy)]
enum Vital {
    #[default]
    HpFill,
    HpText,
    SpFill,
    SpText,
}

/// A region [`rebuild`] refills when its view changes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Host {
    /// Beside the title: the owner, or what the server last said.
    #[default]
    Lead,
    /// What the window says while there is no companion.
    Body,
    Portrait,
    Name,
    Care,
    Orders,
    Tabs,
    Page,
    /// The prompt before deleting or overfeeding.
    Confirm,
    /// The plate on the HUD.
    Hud,
}

const HOSTS: usize = 10;

/// What a control of the window or the plate does.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
enum Act {
    #[default]
    Close,
    Open,
    /// Asks the server for the companion again.
    Refresh,
    Tab(HomunculusTab),
    Follow,
    Standby,
    /// Waits for a cell to walk to.
    Move,
    /// Waits for a target to attack.
    Attack,
    Feed,
    Rest,
    StartRename,
    SaveName,
    CancelName,
    AskDelete,
    /// Flips the "I understand" switch of a deletion.
    Sure,
    Confirm,
    Cancel,
    Cast(u32),
    /// Steps the level a skill is cast at.
    Rank(u32),
    Learn(u32),
    Stance(Stance),
    Flip(Flag),
    Mode(u32, Mode),
    /// Moves a skill's rule one place up.
    Raise(u32),
    /// Adds the typed monster to a list.
    AddMob(MobList),
    DropMob(MobList, u32),
    Undo,
    Teach,
    /// Shrinks the HUD plate to the portrait, or brings it back.
    TogglePlate,
}

#[derive(Clone, Copy)]
struct Confirmation {
    id: u64,
    delete: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
enum HomunculusTab {
    #[default]
    Status,
    Skills,
    Tactics,
}

#[derive(Resource, Default)]
struct OwnerUi {
    tab: HomunculusTab,
    skill_ranks: HashMap<u32, u32>,
    tactics: Editor,
    open: bool,
    was_open: bool,
    confirmation: Option<Confirmation>,
    /// The "I understand" switch of a deletion.
    sure: bool,
    renaming: bool,
    /// A slider is held, so the page is not redrawn under it.
    dragging: bool,
}

/// Spawns the window, the prompt layer and the plate's spot under the HUD root.
pub fn spawn_window(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(ChildOf(parent));
    commands
        .spawn_scene(scene::confirm_host())
        .insert(ChildOf(parent));
    commands.spawn_scene(hud::host()).insert(ChildOf(parent));
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

/// The plate's order keys.
fn order_keys(actions: Query<&ActionState<PlayerAction>, With<LocalPlayer>>, mut desk: Desk) {
    let Ok(keys) = actions.single() else {
        return;
    };
    let orders = [
        (PlayerAction::HomunculusFollow, Act::Follow),
        (PlayerAction::HomunculusAttack, Act::Attack),
        (PlayerAction::HomunculusStandby, Act::Standby),
        (PlayerAction::HomunculusFeed, Act::Feed),
    ];
    for (key, act) in orders {
        if keys.just_pressed(&key) {
            desk.run(act);
        }
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

/// The level a skill is cast at: the picked one, capped at what is learned.
fn selected_rank(ui: &OwnerUi, id: u32, learned: u32) -> u32 {
    ui.skill_ranks
        .get(&id)
        .copied()
        .unwrap_or(learned)
        .min(learned)
}

/// The name to send: trimmed, and neither empty nor the one it already has.
fn new_name(raw: &str, current: &str) -> Option<String> {
    let name = raw.trim();
    (!name.is_empty() && name != current).then(|| name.to_owned())
}

/// Everything a control reads or changes.
#[derive(SystemParam)]
struct Desk<'w, 's> {
    ui: ResMut<'w, OwnerUi>,
    state: Res<'w, HomunculusState>,
    /// Absent in a host without field targeting, where Move and Attack do nothing.
    targeting: Option<ResMut<'w, TargetingMode>>,
    /// Absent in a host without the HUD's saved preferences.
    hud: Option<ResMut<'w, HudSettings>>,
    commands: Commands<'w, 's>,
    out: MessageWriter<'w, HomunculusIntent>,
    casts: MessageWriter<'w, HomunculusCastIntent>,
    names: Query<'w, 's, &'static EditableText, With<NameField>>,
    mobs: Query<'w, 's, &'static mut EditableText, (With<MobField>, Without<NameField>)>,
}

impl Desk<'_, '_> {
    fn run(&mut self, act: Act) {
        match act {
            Act::Close => self.ui.open = false,
            Act::Open => self.ui.open = true,
            Act::Refresh => {
                self.out.write(HomunculusIntent::Inspect);
            }
            Act::Tab(tab) => self.ui.tab = tab,
            Act::StartRename => self.ui.renaming = true,
            Act::CancelName => self.ui.renaming = false,
            Act::Cancel => self.ui.confirmation = None,
            Act::Sure => self.ui.sure ^= true,
            Act::TogglePlate => {
                if let Some(hud) = self.hud.as_mut() {
                    hud.homunculus_minimal ^= true;
                    self.commands.queue(SaveSettings::IfChanged);
                }
            }
            Act::Cast(id) | Act::Rank(id) => self.skill(id, act),
            Act::Stance(stance) => self.ui.tactics.set_stance(stance),
            Act::Flip(flag) => self.ui.tactics.flip(flag),
            Act::Mode(id, mode) => self.ui.tactics.set_mode(id, mode),
            Act::Raise(id) => self.ui.tactics.raise(id),
            Act::DropMob(list, id) => self.ui.tactics.drop_mob(list, id),
            Act::AddMob(list) => self.add_mob(list),
            Act::Undo => {
                let config = self
                    .state
                    .snapshot()
                    .and_then(|s| Some((s.durable_id, s.ai_config.as_ref()?)));
                if let Some((id, config)) = config {
                    self.ui.tactics.reset(id, config);
                }
            }
            request => self.request(request),
        }
    }

    /// Casts a learned skill, or steps the level it is cast at down and around.
    fn skill(&mut self, id: u32, act: Act) {
        let learned = self
            .state
            .snapshot()
            .and_then(|s| s.skills.iter().find(|skill| skill.skill_id == id))
            .map_or(0, |skill| skill.level);
        if learned == 0 {
            return;
        }
        let rank = selected_rank(&self.ui, id, learned);
        if matches!(act, Act::Cast(_)) {
            self.casts.write(HomunculusCastIntent {
                skill_id: id,
                level: Some(rank),
            });
            return;
        }
        let next = if rank == 1 { learned } else { rank - 1 };
        self.ui.skill_ranks.insert(id, next);
    }

    fn add_mob(&mut self, list: MobList) {
        let Ok(mut field) = self.mobs.single_mut() else {
            return;
        };
        let Some(id) = typed_number(&field) else {
            return;
        };
        self.ui.tactics.add_mob(list, id);
        set_field_text(&mut field, "");
    }

    /// Sends what needs the server. Nothing is sent while a request is in flight.
    fn request(&mut self, act: Act) {
        if !self.state.can_mutate() {
            return;
        }
        let Some(current) = self.state.snapshot() else {
            return;
        };
        let durable_id = current.durable_id;
        let action = match act {
            Act::Move | Act::Attack => {
                let (Some(gid), Some(actor), Some(targeting)) = (
                    self.state.controlled_gid(),
                    self.state.controlled_actor(),
                    self.targeting.as_mut(),
                ) else {
                    return;
                };
                **targeting = if act == Act::Move {
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
            Act::Follow => Action::Follow,
            Act::Standby => Action::Standby,
            Act::Rest => Action::Rest,
            Act::Feed if current.hunger <= view::FULL => Action::Feed,
            Act::Feed | Act::AskDelete => {
                if act == Act::Feed && self.state.controlled_gid().is_none() {
                    return;
                }
                self.ui.confirmation = Some(Confirmation {
                    id: durable_id,
                    delete: act == Act::AskDelete,
                });
                self.ui.sure = false;
                return;
            }
            Act::SaveName if current.rename_eligible => {
                let typed = self.names.single().map(|field| field.value().to_string());
                let Some(name) = typed.ok().and_then(|raw| new_name(&raw, &current.name)) else {
                    return;
                };
                self.ui.renaming = false;
                Action::Rename { name }
            }
            Act::Confirm => {
                let Some(pending) = self.ui.confirmation else {
                    return;
                };
                if pending.delete && !self.ui.sure {
                    return;
                }
                self.ui.confirmation = None;
                let Some(action) = confirm(pending, current) else {
                    return;
                };
                action
            }
            Act::Learn(skill_id) => {
                let learnable = current
                    .skills
                    .iter()
                    .any(|skill| skill.skill_id == skill_id && skill.learnable);
                if !learnable || current.skill_points == 0 {
                    return;
                }
                Action::LearnSkill { skill_id }
            }
            Act::Teach => {
                let Some(config) = current.ai_config.as_ref() else {
                    return;
                };
                if !tactics::known(config) || self.ui.tactics.stale(config) {
                    return;
                }
                let Some(config) = self.ui.tactics.teach() else {
                    return;
                };
                Action::ReplaceAi { config }
            }
            _ => return,
        };
        self.out.write(HomunculusIntent::Act { durable_id, action });
    }
}

/// Runs the [`Act`] of an activated button. The headless button also activates on a
/// right click, which is ignored.
fn on_button(
    activate: On<Activate>,
    acts: Query<&Act>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut desk: Desk,
) {
    if mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Right)) {
        return;
    }
    if let Ok(act) = acts.get(activate.entity) {
        desk.run(*act);
    }
}

/// Runs the [`Act`] of a clicked tab, segment, switch or link.
fn on_click(click: On<PointerClick>, acts: Query<&Act>, mut desk: Desk) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Ok(act) = acts.get(click.entity) {
        desk.run(*act);
    }
}

/// Carries a castable skill from its icon to the hotbar.
fn drag_skill(
    drag: On<PointerDragStart>,
    icons: Query<&SkillIcon>,
    state: Res<HomunculusState>,
    mut carried: ResMut<HotbarDrag>,
) {
    let Ok(&SkillIcon(id)) = icons.get(drag.entity) else {
        return;
    };
    let castable = matches!(
        homunculus_skill_spec(id),
        Some(HomunculusSkillSpec::SelfTarget | HomunculusSkillSpec::Enemy)
    );
    let learned = state.snapshot().is_some_and(|s| {
        s.skills
            .iter()
            .any(|skill| skill.skill_id == id && skill.level > 0)
    });
    if castable && learned {
        carried.payload = Some(HotbarSlot::HomunculusSkill(id));
    }
}

type DialRails<'w, 's> =
    Query<'w, 's, (&'static SliderRail<Dial>, &'static RelativeCursorPosition)>;

/// Sets the dial from where the pointer is along its rail.
fn set_dial_from_cursor(rail: Entity, rails: &DialRails, ui: &mut OwnerUi) {
    let Ok((rail, cursor)) = rails.get(rail) else {
        return;
    };
    let Some(at) = slider_at(cursor) else {
        return;
    };
    ui.tactics.set_dial(rail.0, at);
    ui.dragging = true;
}

fn on_dial_press(press: On<PointerPress>, rails: DialRails, mut ui: ResMut<OwnerUi>) {
    set_dial_from_cursor(press.entity, &rails, &mut ui);
}

fn on_dial_drag(drag: On<PointerDrag>, rails: DialRails, mut ui: ResMut<OwnerUi>) {
    set_dial_from_cursor(drag.entity, &rails, &mut ui);
}

/// Lets the page redraw again once the slider is let go.
fn end_drag(mouse: Option<Res<ButtonInput<MouseButton>>>, mut ui: ResMut<OwnerUi>) {
    let held = mouse.is_some_and(|mouse| mouse.pressed(MouseButton::Left));
    if ui.dragging && !held {
        ui.dragging = false;
    }
}

/// Shows or hides the window and keeps the local state in step with the server's.
fn update_window(
    mut ui: ResMut<OwnerUi>,
    state: Res<HomunculusState>,
    mut windows: Query<&mut Node, (With<HomunculusWindow>, Without<CompanionLayout>)>,
    mut layouts: Query<&mut Node, With<CompanionLayout>>,
    menus: Query<Entity, With<OwnerMenu>>,
    mut commands: Commands,
    mut out: MessageWriter<HomunculusIntent>,
) {
    let shown = |shown| if shown { Display::Flex } else { Display::None };
    let laid_out = shown(state.snapshot().is_some());
    if let Ok(mut layout) = layouts.single_mut()
        && layout.display != laid_out
    {
        layout.display = laid_out;
    }
    if ui.open != ui.was_open {
        if let Ok(mut window) = windows.single_mut() {
            window.display = shown(ui.open);
        }
        out.write(HomunculusIntent::WindowVisibility(ui.open));
        ui.was_open = ui.open;
        ui.renaming = false;
        for menu in &menus {
            commands.entity(menu).try_despawn();
        }
    }
    let snapshot = state.snapshot();
    let orphaned = ui.confirmation.is_some_and(|pending| {
        !state.can_mutate() || snapshot.is_none_or(|s| s.durable_id != pending.id)
    });
    if orphaned {
        ui.confirmation = None;
    }
    // A draft lives as long as the window is open.
    let config = snapshot
        .filter(|_| ui.open)
        .and_then(|s| Some((s.durable_id, s.ai_config.as_ref()?)));
    match config {
        Some((id, config)) => ui.tactics.follow(id, config),
        None if ui.tactics.draft().is_some() => ui.tactics = Editor::default(),
        None => {}
    }
}

/// Writes each held or redrawn slider's fill, knob and readout from the draft.
fn sync_dials(
    ui: Res<OwnerUi>,
    mut parts: Query<(&SliderPart, &SliderOf<Dial>, &mut Node, Option<&mut Text>)>,
) {
    let Some(draft) = ui.tactics.draft() else {
        return;
    };
    for (part, of, node, text) in &mut parts {
        if *part != SliderPart::Readout {
            place_slider(*part, node, of.0.fraction(draft));
            continue;
        }
        let readout = of.0.readout(draft);
        if let Some(mut text) = text
            && text.0 != readout
        {
            text.0 = readout;
        }
    }
}

fn sync_cooldowns(
    state: Res<HomunculusState>,
    time: Res<Time<Real>>,
    mut marks: Query<(&Cooldown, &mut Text)>,
) {
    for (mark, mut text) in &mut marks {
        let left = state
            .snapshot()
            .and_then(|s| s.cooldowns.iter().find(|c| c.skill_id == mark.0))
            .map_or(0, |c| state.remaining(c.remaining_ms, time.elapsed()));
        let label = if left > 0 {
            left.div_ceil(1000).to_string()
        } else {
            String::new()
        };
        if text.0 != label {
            text.0 = label;
        }
    }
}

fn sync_vitals(
    state: Res<HomunculusState>,
    mut vitals: Query<(&Vital, &mut Node, Option<&mut Text>)>,
) {
    let Some(s) = state.snapshot() else {
        return;
    };
    for (vital, mut node, text) in &mut vitals {
        let fill = match vital {
            Vital::HpFill | Vital::HpText => s.hp as f32 / s.max_hp.max(1) as f32,
            Vital::SpFill | Vital::SpText => s.sp as f32 / s.max_sp.max(1) as f32,
        };
        let fill = (fill * 100.0).min(100.0);
        let Some(mut text) = text else {
            let width = Val::Percent(fill);
            if node.width != width {
                node.width = width;
            }
            continue;
        };
        let label = format!("{}%", fill.round());
        if text.0 != label {
            text.0 = label;
        }
    }
}

/// What a [`Host`] shows.
#[derive(Clone, PartialEq)]
enum Content {
    Lead(String, Color),
    Body(view::BodyView),
    Portrait(view::PortraitView),
    Name(view::NameView),
    Care(view::CareView),
    Orders(view::OrdersView),
    Tabs(view::TabsView),
    Page(view::PageView),
    Confirm(view::ConfirmView),
    Hud(view::HudView),
}

impl Content {
    fn spawn(self, commands: &mut Commands) -> Entity {
        match self {
            Content::Lead(text, tone) => commands.spawn_scene(scene::lead(text, tone)).id(),
            Content::Body(view) => commands.spawn_scene(scene::body(view)).id(),
            Content::Portrait(view) => commands.spawn_scene(scene::portrait(view)).id(),
            Content::Name(view) => commands.spawn_scene(scene::name(view)).id(),
            Content::Care(view) => commands.spawn_scene(scene::care(view)).id(),
            Content::Orders(view) => commands.spawn_scene(scene::orders(view)).id(),
            Content::Tabs(view) => commands.spawn_scene(scene::tabs(view)).id(),
            Content::Page(view) => commands.spawn_scene(scene::page(view)).id(),
            Content::Confirm(view) => commands.spawn_scene(scene::confirm(view)).id(),
            Content::Hud(view) => commands.spawn_scene(hud::plate(view)).id(),
        }
    }
}

/// The world as the window reads it.
#[derive(SystemParam)]
struct Seen<'w, 's> {
    ui: Res<'w, OwnerUi>,
    state: Res<'w, HomunculusState>,
    time: Res<'w, Time<Real>>,
    skills: Option<Res<'w, SkillCatalog>>,
    items: Option<Res<'w, ItemDb>>,
    inventory: Option<Res<'w, Inventory>>,
    keybinds: Option<Res<'w, Keybinds>>,
    hud: Option<Res<'w, HudSettings>>,
    hero: Hero<'w, 's>,
}

impl Seen<'_, '_> {
    fn ctx(&self, snapshot: &HomunculusSnapshot) -> view::Ctx<'_> {
        let food = self.inventory.as_deref().map_or(0, |bag| {
            bag.iter()
                .filter(|item| item.item_id == snapshot.food_item_id)
                .map(|item| u32::from(item.amount))
                .sum()
        });
        let left = self
            .state
            .remaining(snapshot.active_remaining_ms, self.time.elapsed());
        let refusal = match self.state.last_feedback() {
            Some(HomunculusFeedbackKind::Rejected(error)) => Some(error_text(*error)),
            _ => None,
        };
        view::Ctx {
            ui: &self.ui,
            skills: self.skills.as_deref(),
            items: self.items.as_deref(),
            food,
            out: self.state.controlled_gid().is_some(),
            active_left: Duration::from_millis(left),
            refusal,
            plate_minimal: self
                .hud
                .as_deref()
                .is_some_and(|hud| hud.homunculus_minimal),
        }
    }

    /// The keys of the plate's four orders, short enough for a button's corner.
    fn order_keys(&self) -> [String; 4] {
        let Some(keys) = self.keybinds.as_deref() else {
            return Default::default();
        };
        [
            &keys.homunculus_follow,
            &keys.homunculus_attack,
            &keys.homunculus_standby,
            &keys.homunculus_feed,
        ]
        .map(|binds| keycap_label(&binds.primary).replace(" + ", " "))
    }

    /// What the server last said, else whose companion this is.
    fn lead(&self) -> (String, Color) {
        if let Some(said) = notice(&self.state) {
            return said;
        }
        let owner = self
            .state
            .snapshot()
            .and_then(|_| self.hero.identity())
            .map_or_else(String::new, |(name, ..)| {
                format!("{name}\u{2019}s companion")
            });
        (owner, ro_dark::MUTED)
    }

    /// What `host` shows now. The window's hosts are empty while it is closed.
    fn content(&self, host: Host) -> Option<Content> {
        let open = self.ui.open;
        let snapshot = self.state.snapshot();
        let shown = snapshot.filter(|_| open || matches!(host, Host::Confirm | Host::Hud));
        match host {
            Host::Lead => open.then(|| {
                let (text, tone) = self.lead();
                Content::Lead(text, tone)
            }),
            Host::Body => view::body(self.state.presence())
                .filter(|_| open)
                .map(Content::Body),
            inner => {
                let s = shown?;
                let ctx = self.ctx(s);
                Some(match inner {
                    Host::Portrait => Content::Portrait(view::portrait(s)),
                    Host::Name => Content::Name(view::name(s, &ctx)),
                    Host::Care => Content::Care(view::care(s, &ctx)),
                    Host::Orders => Content::Orders(view::orders(s, &ctx)),
                    Host::Tabs => Content::Tabs(view::tabs(s, &ctx)),
                    Host::Page => Content::Page(view::page(s, &ctx)),
                    Host::Confirm => {
                        Content::Confirm(view::confirm(self.ui.confirmation?, s, &ctx))
                    }
                    _ => Content::Hud(view::hud(s, &ctx, self.order_keys())?),
                })
            }
        }
    }
}

/// Refills every [`Host`] whose view changed. The page waits while a slider is held.
fn rebuild(
    mut commands: Commands,
    seen: Seen,
    hosts: Query<(Entity, Ref<Host>, Option<&Children>)>,
    mut shown: Local<[Option<Content>; HOSTS]>,
) {
    for (host, kind, children) in &hosts {
        if *kind == Host::Page && seen.ui.dragging {
            continue;
        }
        let content = seen.content(*kind);
        let slot = &mut shown[*kind as usize];
        if *slot == content && !kind.is_added() {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        if let Some(content) = content.clone() {
            let spawned = content.spawn(&mut commands);
            commands.entity(spawned).insert(ChildOf(host));
        }
        *slot = content;
    }
}

/// What the server last said about a request, and the tone to say it in.
fn notice(state: &HomunculusState) -> Option<(String, Color)> {
    Some(match state.last_feedback() {
        Some(HomunculusFeedbackKind::Rejected(error)) => {
            (error_text(*error).into(), ro_dark::TONE_ERROR)
        }
        Some(HomunculusFeedbackKind::SendFailed(reason)) => (reason.clone(), ro_dark::TONE_ERROR),
        Some(HomunculusFeedbackKind::TimedOut) => (
            "No reply received. Refresh to try again.".into(),
            ro_dark::STATUS_BUSY,
        ),
        Some(HomunculusFeedbackKind::Removed) => (
            "Your companion was permanently removed.".into(),
            ro_dark::MUTED,
        ),
        _ if state.pending_mutation().is_some() => {
            ("Waiting for the server\u{2026}".into(), ro_dark::MUTED)
        }
        _ => return None,
    })
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
