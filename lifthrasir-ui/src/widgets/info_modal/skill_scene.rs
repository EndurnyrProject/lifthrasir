//! Skill info modal content: renders a [`SkillInfoView`] through the shell chrome —
//! header (kind + state tag) → level pips → meta grid (SP cost, range) → colored
//! description → Requires/Unlocks chips. Chip clicks write [`ShowInfoModal`] for
//! their skill id; rebuilding the modal on that message is the whole navigation
//! model — there is no back stack.
//!
//! The footer Raise button stages `+1` in [`SkillPanelStaging`] — the same path the
//! skills-tab stepper uses (`skills_tab.rs::on_stepper`) — and never writes
//! `SkillLearnRequested` directly; the tab's own Apply flow is the only place that
//! batches staged levels to the server.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;

use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::skill::SkillTreeState;

use crate::theme;
use crate::widgets::character_window::SkillPanelStaging;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::shell::{self, EdgeGrade, HeaderView};
use super::view::{SkillInfoView, SkillReqChip};
use super::{InfoTarget, ShowInfoModal};

/// Parses a `SkillInfoView::level_line` (`"cur/max"`, always written by
/// `build_skill_view`) back into its parts. `(0, 0)` for anything malformed rather
/// than panicking — display-only, never gates staging.
fn parse_level_line(line: &str) -> (u32, u32) {
    let mut parts = line.splitn(2, '/');
    let cur = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let max = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (cur, max)
}

/// Mastered/Locked/Learned/Available, per the mockup's header state tag. Derived
/// from `view.edge` (the same grading `skill_edge_grade` produced the ribbon color
/// from) rather than re-deriving the maxed/unmet predicates, so the tag can never
/// desync from the ribbon.
fn state_tag(edge: EdgeGrade, cur: u32) -> &'static str {
    match edge {
        EdgeGrade::Rare => "Mastered",
        EdgeGrade::Common => "Locked",
        _ if cur > 0 => "Learned",
        _ => "Available",
    }
}

/// Carries the target skill id for a Requires/Unlocks chip's click observer.
#[derive(Component, Clone, Copy)]
struct ChipTarget(u32);

/// Always set explicitly via `template_value` at the chip call site; exists only so
/// `ChipTarget` satisfies `bsn!`'s `Template` bound.
impl Default for ChipTarget {
    fn default() -> Self {
        Self(0)
    }
}

/// Carries the skill id and disabled state a Raise button acts on.
#[derive(Component, Clone, Copy)]
pub(super) struct RaiseAction {
    skill_id: u32,
    disabled: bool,
}

/// Always set explicitly via `template_value` at the button call site; exists only
/// so `RaiseAction` satisfies `bsn!`'s `Template` bound.
impl Default for RaiseAction {
    fn default() -> Self {
        Self {
            skill_id: 0,
            disabled: false,
        }
    }
}

/// The skill modal's whole content: header, then the section stack, then the
/// always-present Raise footer.
pub(super) fn scene(view: SkillInfoView, skill_id: u32) -> impl Scene {
    let (cur, max) = parse_level_line(&view.level_line);
    let tag = state_tag(view.edge, cur).to_string();

    let header = shell::header(HeaderView {
        icon_path: view.icon_path.clone(),
        refine: None,
        sockets_filled: 0,
        sockets_total: 0,
        edge: view.edge,
        name: view.name.clone(),
        tags: vec![view.kind.clone(), tag],
    });

    let mut meta_cells = Vec::new();
    if let Some(sp) = view.sp_cost.clone() {
        meta_cells.push(shell::meta_cell("SP Cost".to_string(), sp));
    }
    if let Some(range) = view.range.clone() {
        meta_cells.push(shell::meta_cell("Range".to_string(), range));
    }
    let meta = (!meta_cells.is_empty()).then(|| EntityScene(shell::meta_grid(meta_cells)));

    let description = (!view.description.is_empty())
        .then(|| EntityScene(shell::description_section(view.description.clone())));

    let requires = (!view.requires.is_empty()).then(|| {
        EntityScene(chip_section(
            "Requires".to_string(),
            view.requires.clone(),
            true,
        ))
    });
    let unlocks = (!view.unlocks.is_empty()).then(|| {
        EntityScene(chip_section(
            "Unlocks".to_string(),
            view.unlocks.clone(),
            false,
        ))
    });

    let raise_disabled = !view.can_raise || view.points_left == 0;
    let raise_label = if max > 0 && cur >= max {
        "Mastered".to_string()
    } else {
        format!("Raise to Lv {}", cur + 1)
    };

    bsn! {
        Node { flex_direction: FlexDirection::Column, min_height: px(0) }
        ignore_picking()
        Children [
            header,
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(14),
                    padding: {UiRect { left: px(20), right: px(20), top: px(0), bottom: px(6) }},
                }
                ignore_picking()
                Children [
                    level_pips(cur, max),
                    {meta},
                    {description},
                    {requires},
                    {unlocks},
                ]
            ),
            shell::footer_bar(vec![raise_button(skill_id, raise_label, raise_disabled)]),
        ]
    }
}

/// `LEVEL [pips] cur/max` — `.im-lv`.
fn level_pips(cur: u32, max: u32) -> impl Scene {
    let pips: Vec<_> = (0..max).map(|i| pip(i < cur)).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(11) }
        ignore_picking()
        Children [
            chrome_text("LEVEL".to_string(), 9.5, theme::TEXT_FAINT),
            (
                Node { flex_direction: FlexDirection::Row, flex_grow: 1.0, column_gap: px(3) }
                ignore_picking()
                Children [ {pips} ]
            ),
            chrome_text(format!("{cur}/{max}"), 12.0, theme::TEXT),
        ]
    }
}

fn pip(filled: bool) -> impl Scene {
    let color = if filled {
        theme::EMERALD_BRI
    } else {
        theme::STROKE_STRONG
    };
    bsn! {
        Node { flex_grow: 1.0, height: px(7), border_radius: BorderRadius::all(px(2)) }
        BackgroundColor(color)
        ignore_picking()
    }
}

/// A Requires or Unlocks section: a label followed by wrapping chips.
fn chip_section(label: String, chips: Vec<SkillReqChip>, is_requires: bool) -> impl Scene {
    let chip_scenes: Vec<_> = chips
        .into_iter()
        .map(|chip| skill_chip(chip, is_requires))
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        ignore_picking()
        Children [
            shell::section_label(label, None),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(7),
                    row_gap: px(7),
                }
                ignore_picking()
                Children [ {chip_scenes} ]
            ),
        ]
    }
}

/// One chip — `.im-chip`. Requires chips grade red/unmet or green/met; unlock chips
/// stay neutral. Clicking navigates the modal to `chip.skill_id` by rebuilding it.
fn skill_chip(chip: SkillReqChip, is_requires: bool) -> impl Scene {
    let text = format!("{} Lv {}", chip.name, chip.level);
    let (bg, border, color) = if !is_requires {
        (
            Color::WHITE.with_alpha(0.03),
            theme::STROKE,
            theme::TEXT_DIM,
        )
    } else if chip.met {
        (
            theme::EMERALD.with_alpha(0.1),
            theme::EMERALD.with_alpha(0.3),
            theme::EMERALD_BRI,
        )
    } else {
        let unmet = Color::srgb_u8(0xe8, 0x94, 0x90);
        (unmet.with_alpha(0.1), unmet.with_alpha(0.28), unmet)
    };
    let skill_id = chip.skill_id;
    bsn! {
        template_value(ChipTarget(skill_id))
        Node {
            padding: {UiRect::axes(px(10), px(5))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
        Pickable
        on(on_chip_click)
        Children [ chrome_text(text, 11.0, color) ]
    }
}

fn on_chip_click(
    click: On<Pointer<Click>>,
    chips: Query<&ChipTarget>,
    mut writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(target) = chips.get(click.entity) else {
        return;
    };
    writer.write(ShowInfoModal {
        target: InfoTarget::Skill(target.0),
    });
}

fn raise_button(skill_id: u32, label: String, disabled: bool) -> impl Scene {
    bsn! {
        template_value(RaiseAction { skill_id, disabled })
        @FeathersButton {
            @caption: bsn! { (Text(label) ThemedText) },
            @variant: ButtonVariant::Primary,
        }
        Node { flex_grow: 1.0, height: px(40), border_radius: BorderRadius::all(px(9)) }
        on(on_raise_click)
    }
}

/// Applies [`RaiseAction::disabled`] as Feathers' `InteractionDisabled` once per
/// spawned button — the modal rebuilds from scratch on every show, so `Added` fires
/// exactly once per button and needs no steady-state upkeep.
pub(super) fn apply_raise_disabled(
    buttons: Query<(Entity, &RaiseAction), Added<RaiseAction>>,
    mut commands: Commands,
) {
    for (entity, action) in &buttons {
        if action.disabled {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
}

/// Stages `+1` for the button's skill via [`SkillPanelStaging::raise`] — the same
/// gated staging call the skills-tab stepper uses. Never writes
/// `SkillLearnRequested`; that only happens from the tab's batched Apply flow.
fn on_raise_click(
    activate: On<Activate>,
    actions: Query<&RaiseAction>,
    tree: Res<SkillTreeState>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<SkillPanelStaging>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    let Ok(status) = player.single() else {
        return;
    };
    staging.raise(action.skill_id, &tree, status, status.skill_point);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn full_view() -> SkillInfoView {
        SkillInfoView {
            icon_path: Some("skills/sm_bash.png".to_string()),
            edge: shell::EdgeGrade::Fine,
            name: "Bash".to_string(),
            kind: "Active".to_string(),
            level_line: "3/10".to_string(),
            description: vec![vec![(theme::TEXT_DIM, "Deals heavy damage.".to_string())]],
            sp_cost: Some("10".to_string()),
            range: Some("1".to_string()),
            requires: vec![SkillReqChip {
                skill_id: 9,
                name: "Endure".to_string(),
                level: 1,
                met: true,
            }],
            unlocks: vec![SkillReqChip {
                skill_id: 17,
                name: "Magnum Break".to_string(),
                level: 5,
                met: false,
            }],
            can_raise: true,
            points_left: 3,
        }
    }

    fn texts(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn full_view_renders_pips_meta_description_and_chips() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(full_view(), 5))
            .expect("scene spawns");
        app.update();

        let texts = texts(&mut app);
        assert!(texts.contains(&"Bash".to_string()), "{texts:?}");
        assert!(texts.contains(&"3/10".to_string()), "{texts:?}");
        assert!(texts.contains(&"SP COST".to_string()), "{texts:?}");
        assert!(texts.contains(&"10".to_string()), "{texts:?}");
        assert!(texts.contains(&"RANGE".to_string()), "{texts:?}");
        assert!(
            texts.contains(&"Deals heavy damage.".to_string()),
            "{texts:?}"
        );
        assert!(texts.contains(&"Endure Lv 1".to_string()), "{texts:?}");
        assert!(
            texts.contains(&"Magnum Break Lv 5".to_string()),
            "{texts:?}"
        );
        assert!(texts.contains(&"Raise to Lv 4".to_string()), "{texts:?}");
    }

    #[test]
    fn requirement_chip_click_emits_show_info_modal_for_its_skill() {
        let mut app = App::new();
        app.add_message::<ShowInfoModal>();
        let window = app.world_mut().spawn_empty().id();
        let chip = app
            .world_mut()
            .spawn(ChipTarget(9))
            .observe(on_chip_click)
            .id();

        app.world_mut().trigger(click_event(chip, window));
        app.world_mut().flush();

        let messages: Vec<_> = app
            .world()
            .resource::<Messages<ShowInfoModal>>()
            .iter_current_update_messages()
            .map(|m| m.target)
            .collect();
        assert_eq!(messages.len(), 1);
        assert!(matches!(messages[0], InfoTarget::Skill(9)));
    }

    fn click_event(target: Entity, window: Entity) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button: PointerButton::Primary,
                hit: HitData::new(target, 0.0, None, None),
                duration: std::time::Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    fn raise_test_app() -> App {
        let mut app = App::new();
        app.add_message::<game_engine::domain::entities::character::events::SkillLearnRequested>();
        app.init_resource::<SkillTreeState>();
        app.init_resource::<SkillPanelStaging>();
        app
    }

    fn seed_tree(app: &mut App, skill_id: u32, max_level: u32) {
        use game_engine::domain::skill::SkillNode;
        app.world_mut()
            .resource_mut::<SkillTreeState>()
            .skills
            .insert(
                skill_id,
                SkillNode {
                    level: 0,
                    max_level,
                    upgradable: true,
                    requires: vec![],
                    req_base_level: 0,
                    req_job_level: 0,
                    sp: 1,
                    range: 1,
                    inf_type: 0,
                    job_id: 1,
                    splash_radius: 0,
                },
            );
    }

    #[test]
    fn raise_click_stages_a_level_and_never_writes_skill_learn_requested() {
        let mut app = raise_test_app();
        seed_tree(&mut app, 5, 10);
        app.world_mut().spawn((
            CharacterStatus {
                base_level: 1,
                job_level: 1,
                skill_point: 3,
                ..Default::default()
            },
            LocalPlayer,
        ));
        let button = app
            .world_mut()
            .spawn(RaiseAction {
                skill_id: 5,
                disabled: false,
            })
            .observe(on_raise_click)
            .id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        assert_eq!(app.world().resource::<SkillPanelStaging>().staged(5), 1);
        let learned = app
            .world()
            .resource::<Messages<game_engine::domain::entities::character::events::SkillLearnRequested>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(learned, 0);
    }

    #[test]
    fn raise_button_carries_disabled_state_when_no_points_left() {
        let mut app = test_app();
        let mut view = full_view();
        view.can_raise = false;
        view.points_left = 0;
        app.world_mut()
            .spawn_scene(scene(view, 5))
            .expect("scene spawns");
        app.update();

        let actions: Vec<_> = app
            .world_mut()
            .query::<&RaiseAction>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(actions.len(), 1);
        assert!(actions[0].disabled);
    }

    #[test]
    fn apply_raise_disabled_inserts_interaction_disabled_only_when_flagged() {
        let mut app = App::new();
        app.add_systems(Update, apply_raise_disabled);
        let disabled = app
            .world_mut()
            .spawn(RaiseAction {
                skill_id: 1,
                disabled: true,
            })
            .id();
        let enabled = app
            .world_mut()
            .spawn(RaiseAction {
                skill_id: 2,
                disabled: false,
            })
            .id();
        app.update();

        assert!(app.world().get::<InteractionDisabled>(disabled).is_some());
        assert!(app.world().get::<InteractionDisabled>(enabled).is_none());
    }
}
