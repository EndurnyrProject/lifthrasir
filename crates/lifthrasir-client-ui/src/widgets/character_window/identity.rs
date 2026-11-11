//! The Character window header: name, class and levels beside the HP, SP, Base and
//! Job bars, the weight and zeny read-out, plus an AP bar for 4th-job characters with
//! a non-zero `max_ap`. It is a pure projection of the `LocalPlayer`'s
//! `EntityName`/`CharacterData`/`CharacterStatus` into the shell's
//! [`CharacterIdentityMount`], respawned by [`rebuild_identity`] on any change.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use lifthrasir_assets::catalogs::job::player_jobs::is_fourth_job;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_client::domain::entities::character::components::core::CharacterData;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::display_text;

use crate::screens::shared::thousands;
use crate::widgets::character_window::CharacterIdentityMount;

const NAME_BLOCK: f32 = 232.0;
const BAR_HEIGHT: f32 = 16.0;

/// Marks the AP bar's row so it is distinguishable from HP/SP; present only when
/// the local player is a 4th job with `max_ap > 0`.
#[derive(Component, Default, Clone)]
pub struct ApMeter;

/// `cur/max` as a `0.0..=1.0` fraction; a zero `max` reads as empty.
fn fraction(cur: u32, max: u32) -> f32 {
    if max == 0 {
        0.0
    } else {
        cur as f32 / max as f32
    }
}

/// The header's owned view-model, prepared before the `bsn!` block (scenes own their
/// data). `ap` is `Some` only when the AP bar should render; the exp fields are
/// `0.0..=1.0` fractions of the next level.
struct IdentityView {
    name: String,
    class: String,
    base_level: u32,
    job_level: u32,
    zeny: u32,
    weight: u32,
    max_weight: u32,
    hp: u32,
    max_hp: u32,
    sp: u32,
    max_sp: u32,
    ap: Option<(u32, u32)>,
    base_exp: f32,
    job_exp: f32,
}

impl IdentityView {
    fn build(
        status: &CharacterStatus,
        data: &CharacterData,
        entity_name: Option<&EntityName>,
        registry: Option<&JobSpriteRegistry>,
    ) -> Self {
        let name = entity_name
            .map(|n| n.name.clone())
            .unwrap_or_else(|| data.name.clone());
        let class = registry
            .and_then(|registry| registry.get_display_name(data.job_id as u32))
            .unwrap_or("Unknown")
            .to_string();
        let ap = (is_fourth_job(data.job_id as u32) && status.max_ap > 0)
            .then_some((status.ap, status.max_ap));
        Self {
            name,
            class,
            base_level: status.base_level,
            job_level: status.job_level,
            zeny: status.zeny,
            weight: status.weight,
            max_weight: status.max_weight,
            hp: status.hp,
            max_hp: status.max_hp,
            sp: status.sp,
            max_sp: status.max_sp,
            ap,
            base_exp: fraction(status.base_exp, status.next_base_exp),
            job_exp: fraction(status.job_exp, status.next_job_exp),
        }
    }
}

type ChangedIdentity = (
    With<LocalPlayer>,
    Or<(
        Changed<CharacterStatus>,
        Changed<CharacterData>,
        Changed<EntityName>,
    )>,
);

/// Gates [`rebuild_identity`]: run only when the local player's status, data, or
/// name change, when the job registry loads (so "Unknown" resolves to a real job
/// name), or when the identity mount is freshly spawned.
pub fn identity_changed(
    player: Query<(), ChangedIdentity>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mounts: Query<(), Added<CharacterIdentityMount>>,
) -> bool {
    !player.is_empty()
        || job_registry.is_some_and(|registry| registry.is_changed())
        || !mounts.is_empty()
}

/// Respawns the [`CharacterIdentityMount`]'s children from the local player's
/// live state (the `inventory_window` rebuild-body pattern). A missing player or
/// mount skips silently; the next change retries.
pub fn rebuild_identity(
    mut commands: Commands,
    player: Query<(&CharacterStatus, &CharacterData, Option<&EntityName>), With<LocalPlayer>>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mounts: Query<(Entity, Option<&Children>), With<CharacterIdentityMount>>,
) {
    let Ok((status, data, entity_name)) = player.single() else {
        return;
    };
    let Ok((mount, children)) = mounts.single() else {
        return;
    };

    let view = IdentityView::build(status, data, entity_name, job_registry.as_deref());

    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(identity_content(view))
        .insert(ChildOf(mount));
}

/// `cur / max` with thousands separators.
fn out_of(cur: u32, max: u32) -> String {
    format!("{} / {}", thousands(cur.into()), thousands(max.into()))
}

fn percent_label(ratio: f32) -> String {
    format!("{:.1}%", ratio * 100.0)
}

fn identity_content(view: IdentityView) -> impl Scene {
    let ap = view.ap.map(|(ap, max_ap)| {
        EntityScene(bsn! {
            ApMeter
            @bar("AP", fraction(ap, max_ap), ro_dark::BASE_EXP, out_of(ap, max_ap))
        })
    });
    bsn! {
        Node { height: px(60), align_items: AlignItems::Center, column_gap: px(28) }
        @ignore_picking()
        Children [
            @name_block(view.name, view.class, view.base_level, view.job_level)
            --
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), column_gap: px(24) }
            @ignore_picking()
            Children [
                @bar_column()
                Children [
                    @bar("HP", fraction(view.hp, view.max_hp), ro_dark::HP, out_of(view.hp, view.max_hp))
                    --
                    @bar("SP", fraction(view.sp, view.max_sp), ro_dark::SP, out_of(view.sp, view.max_sp))
                    --
                    {ap}
                ]
                --
                @bar_column()
                Children [
                    @bar("Base", view.base_exp, ro_dark::BASE_EXP, percent_label(view.base_exp))
                    --
                    @bar("Job", view.job_exp, ro_dark::JOB_EXP, percent_label(view.job_exp))
                    --
                    @purse(view.weight, view.max_weight, view.zeny)
                ]
            ]
        ]
    }
}

/// The hero's name over the class and levels line.
fn name_block(name: String, class: String, base_level: u32, job_level: u32) -> impl Scene {
    let levels = format!("\u{b7} Base Lv {base_level} \u{b7} Job Lv {job_level}");
    bsn! {
        Node {
            width: px(NAME_BLOCK),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            overflow: {Overflow::clip()},
        }
        @ignore_picking()
        Children [
            @display_text(name, 28.0, ro_dark::HERO)
            TextLayout { linebreak: LineBreak::NoWrap }
            --
            Node { column_gap: px(4) }
            @ignore_picking()
            Children [
                @chrome_text(class, 12.0, ro_dark::BRONZE_LIGHT)
                --
                @chrome_text(levels, 12.0, ro_dark::MUTED)
            ]
        ]
    }
}

/// One of the two stacks of bars beside the name.
fn bar_column() -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
        }
        @ignore_picking()
    }
}

/// A tagged bar: the trough holds a `fill` sized to `ratio` with `value` centred over
/// it.
fn bar(tag: &'static str, ratio: f32, fill: Color, value: String) -> impl Scene {
    let width = Val::Percent(ratio.clamp(0.0, 1.0) * 100.0);
    bsn! {
        Node { height: px(BAR_HEIGHT), align_items: AlignItems::Center, column_gap: px(10) }
        @ignore_picking()
        Children [
            @chrome_text(tag.into(), 12.0, ro_dark::LABEL)
            Node { width: px(34), flex_shrink: 0.0 }
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                height: px(BAR_HEIGHT),
                padding: px(1),
                border: px(1),
                border_radius: BorderRadius::all(px(8)),
                position_type: PositionType::Relative,
            }
            BackgroundColor(ro_dark::BAR_TROUGH)
            BorderColor::all(ro_dark::BAR_FRAME)
            @ignore_picking()
            Children [
                Node {
                    width: {width},
                    height: percent(100),
                    border_radius: BorderRadius::all(px(6)),
                }
                BackgroundColor(fill)
                @ignore_picking()
                --
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                @ignore_picking()
                Children [
                    @chrome_text(value, 10.5, ro_dark::VALUE)
                    TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
                ]
            ]
        ]
    }
}

/// Carried weight and zeny, in the bars' last row.
fn purse(weight: u32, max_weight: u32, zeny: u32) -> impl Scene {
    let zeny = format!("{} z", thousands(zeny.into()));
    bsn! {
        Node { height: px(BAR_HEIGHT), align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            @chrome_text("Weight".into(), 12.0, ro_dark::LABEL)
            --
            @chrome_text(out_of(weight, max_weight), 12.0, ro_dark::STRONG)
            Node { flex_grow: 1.0 }
            --
            @chrome_text("Zeny".into(), 12.0, ro_dark::ACCENT)
            --
            @chrome_text(zeny, 12.0, ro_dark::BRONZE_LIGHT)
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::entities::character::components::core::CharacterStats;

    fn identity_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn spawn_player(app: &mut App, job_id: u16, max_ap: u32) {
        app.world_mut().spawn((
            CharacterStatus {
                hp: 100,
                max_hp: 200,
                sp: 40,
                max_sp: 40,
                ap: 30,
                max_ap,
                base_level: 50,
                job_level: 10,
                zeny: 999,
                weight: 100,
                max_weight: 2000,
                ..default()
            },
            CharacterData {
                name: "Hero".to_string(),
                job_id,
                level: 1,
                experience: 0,
                stats: CharacterStats::default(),
                slot: 0,
            },
            LocalPlayer,
        ));
    }

    fn ap_meter_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<(), With<ApMeter>>()
            .iter(world)
            .count()
    }

    /// True if any spawned `Text` node reads exactly `needle`.
    fn has_text(app: &mut App, needle: &str) -> bool {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == needle)
    }

    #[test]
    fn fraction_guards_zero_max() {
        assert_eq!(fraction(50, 100), 0.5);
        assert_eq!(fraction(1, 0), 0.0);
        assert_eq!(fraction(0, 0), 0.0);
    }

    #[test]
    fn non_trait_job_hides_ap_meter() {
        let mut app = identity_app();
        app.world_mut().spawn(CharacterIdentityMount);
        spawn_player(&mut app, 4001, 0);

        app.add_systems(Update, rebuild_identity);
        app.update();

        assert_eq!(
            ap_meter_count(&mut app),
            0,
            "AP meter hidden for a non-trait job"
        );
        assert!(has_text(&mut app, "100 / 200"), "HP meter still rendered");
    }

    #[test]
    fn trait_job_with_ap_shows_ap_meter() {
        let mut app = identity_app();
        app.world_mut().spawn(CharacterIdentityMount);
        spawn_player(&mut app, 4252, 70);

        app.add_systems(Update, rebuild_identity);
        app.update();

        assert_eq!(
            ap_meter_count(&mut app),
            1,
            "AP meter shown for a 4th job with AP"
        );
        assert!(has_text(&mut app, "100 / 200"), "HP meter still rendered");
    }

    #[test]
    fn trait_job_without_ap_hides_ap_meter() {
        let mut app = identity_app();
        app.world_mut().spawn(CharacterIdentityMount);
        spawn_player(&mut app, 4252, 0);

        app.add_systems(Update, rebuild_identity);
        app.update();

        assert_eq!(
            ap_meter_count(&mut app),
            0,
            "AP meter hidden for a 4th job with zero max_ap"
        );
    }

    #[test]
    fn non_trait_job_with_ap_hides_ap_meter() {
        let mut app = identity_app();
        app.world_mut().spawn(CharacterIdentityMount);
        spawn_player(&mut app, 4001, 70);

        app.add_systems(Update, rebuild_identity);
        app.update();

        assert_eq!(
            ap_meter_count(&mut app),
            0,
            "AP meter hidden for a non-trait job even with max_ap"
        );
    }
}
