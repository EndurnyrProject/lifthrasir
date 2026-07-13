//! The Console identity strip: avatar, name, class, Base/Job level, Zeny, Weight,
//! and HP/SP meters (always) plus an AP meter (only for 4th-job characters with a
//! non-zero `max_ap`). It is a pure projection of the `LocalPlayer`'s
//! `EntityName`/`CharacterData`/`CharacterStatus` into the shell's
//! [`CharacterIdentityMount`], respawned by [`rebuild_identity`] on any change —
//! the same change-gating `character_info` uses, into the mount instead of the HUD
//! frame (and adding AP).

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use game_engine::domain::entities::character::components::core::CharacterData;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::infrastructure::job::player_jobs::is_fourth_job;
use game_engine::infrastructure::job::JobSpriteRegistry;

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::meter::meter;
use super::CharacterIdentityMount;

const AVATAR_BG: Color = Color::srgb_u8(0x1f, 0x2b, 0x25);

/// Marks the AP meter's row so it is distinguishable from HP/SP; present only when
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

/// The identity strip's owned view-model, prepared before the `bsn!` block (scenes
/// own their data). `ap` is `Some` only when the AP meter should render.
struct IdentityView {
    avatar: String,
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
        let avatar = class
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        let ap = (is_fourth_job(data.job_id as u32) && status.max_ap > 0)
            .then_some((status.ap, status.max_ap));
        Self {
            avatar,
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
/// name), or when the identity mount is freshly spawned. Mirrors
/// `character_info`'s `character_info_changed`.
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

fn identity_content(view: IdentityView) -> impl Scene {
    let ap = view
        .ap
        .map(|(ap, max_ap)| EntityScene(ap_line(fraction(ap, max_ap), format!("{ap} / {max_ap}"))));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(9) }
        ignore_picking()
        Children [
            header_row(view.avatar, view.name, view.class, view.base_level, view.job_level),
            resources_row(view.zeny, view.weight, view.max_weight),
            meter_line("HP", fraction(view.hp, view.max_hp), theme::EMERALD_BRI, format!("{} / {}", view.hp, view.max_hp)),
            meter_line("SP", fraction(view.sp, view.max_sp), theme::MANA_BLUE, format!("{} / {}", view.sp, view.max_sp)),
            {ap},
        ]
    }
}

fn header_row(
    avatar: String,
    name: String,
    class: String,
    base_level: u32,
    job_level: u32,
) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(11) }
        ignore_picking()
        Children [
            avatar_box(avatar),
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, min_width: px(0) }
                ignore_picking()
                Children [
                    name_text(name),
                    chrome_text(class, 11.5, theme::GOLD),
                ]
            ),
            level_chip("Base", base_level),
            level_chip("Job", job_level),
        ]
    }
}

fn avatar_box(letter: String) -> impl Scene {
    bsn! {
        Node {
            width: px(40),
            height: px(40),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor(AVATAR_BG)
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [
            (
                Text(letter)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(18.0)},
                }
                TextColor(theme::GOLD)
                ignore_picking()
            )
        ]
    }
}

fn name_text(name: String) -> impl Scene {
    bsn! {
        Text(name)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
            font_size: {FontSize::Px(16.0)},
        }
        TextColor(theme::EMERALD_BRI)
        ignore_picking()
    }
}

fn level_chip(label: &str, level: u32) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center }
        ignore_picking()
        Children [
            chrome_text(label.to_string(), 10.0, theme::TEXT_FAINT),
            chrome_text(level.to_string(), 12.0, theme::TEXT),
        ]
    }
}

fn resources_row(zeny: u32, weight: u32, max_weight: u32) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
        ignore_picking()
        Children [
            stat_pair("Zeny", zeny.to_string()),
            stat_pair("Weight", format!("{weight} / {max_weight}")),
        ]
    }
}

fn stat_pair(label: &str, value: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(6), align_items: AlignItems::Center }
        ignore_picking()
        Children [
            chrome_text(label.to_string(), 11.0, theme::TEXT_DIM),
            chrome_text(value, 11.5, theme::TEXT),
        ]
    }
}

/// One labeled meter line: a fixed-width tag ("HP"/"SP") + the [`meter`] bar.
fn meter_line(tag: &str, ratio: f32, fill: Color, label: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(9) }
        ignore_picking()
        Children [
            (
                Text({tag.to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(10.5)},
                }
                TextColor(theme::TEXT_FAINT)
                Node { width: px(22) }
                ignore_picking()
            ),
            meter(ratio, fill, label),
        ]
    }
}

/// The AP meter line: the shared [`meter_line`] with [`ApMeter`] attached directly
/// to its entity so its presence is queryable, no wrapper node.
fn ap_line(ratio: f32, label: String) -> impl Scene {
    bsn! {
        ApMeter
        meter_line("AP", ratio, theme::GOLD, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::domain::entities::character::components::core::CharacterStats;

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
