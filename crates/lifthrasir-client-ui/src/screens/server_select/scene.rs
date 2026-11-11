//! BSN scenes of the server selection screen: the static frame, the runtime rows,
//! the details card and the connecting modal. Behaviour lives in the parent module.

use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::ScrollArea;
use lifthrasir_ui::theme;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, WINDOW_SHADOW, button, title_text, window_title, window_titled,
};

use super::*;

/// Design sizes, excluding the baked window shadow.
const WINDOW_WIDTH: f32 = 940.0;
const MODAL_WIDTH: f32 = 440.0;
const BODY_HEIGHT: f32 = 424.0;
const LIST_WIDTH: f32 = 520.0;

/// The whole screen: a vignette over the menu background, the header, the window
/// and the server clock.
pub(super) fn server_select_screen(username: String, online: u32) -> impl Scene {
    let vignette = BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::FarthestCorner,
        vec![
            ColorStop::new(Color::srgba_u8(10, 13, 18, 89), percent(0)),
            ColorStop::new(Color::srgba_u8(6, 8, 12, 219), percent(75)),
        ],
    ));
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        vignette
        Children [
            @header(username)
            --
            @window(online)
            --
            ServerClock
            @chrome_text(String::new(), 11.0, ro_dark::FAINT)
            Node { position_type: PositionType::Absolute, right: px(40), bottom: px(16) }
        ]
    }
}

fn header(username: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(26),
            left: px(40),
            right: px(40),
            align_items: AlignItems::Center,
            column_gap: px(6),
        }
        Children [
            @title_text("LIFTHRASIR", 22.0)
            TextShadow { offset: Vec2::splat(2.0), color: {Color::BLACK.with_alpha(0.85)} }
            --
            Node { flex_grow: 1.0 }
            --
            @chrome_text("Signed in as".into(), 12.0, ro_dark::LABEL)
            --
            @chrome_text(username, 12.0, ro_dark::STRONG)
            --
            Node { width: px(96.0 + 2.0 * BUTTON_SHADOW) }
            Children [
                @button(ButtonArt::NORMAL, "Log Out")
                on(back_to_login)
            ]
        ]
    }
}

fn window(online: u32) -> impl Scene {
    bsn! {
        @window_titled()
        Node {
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            max_width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: px(10),
        }
        Children [
            @window_title("Choose a World")
            Node { column_gap: px(12) }
            Children [
                @chrome_text(format!("{} heroes online", thousands(online)), 12.0, ro_dark::MUTED)
            ]
            --
            Node { height: px(BODY_HEIGHT), column_gap: px(18) }
            Children [
                ServerList
                ScrollArea
                @inset()
                Node {
                    width: px(LIST_WIDTH),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    padding: {UiRect::all(px(6))},
                    overflow: {Overflow::scroll_y()},
                }
                --
                DetailsPanel
                @inset()
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    flex_direction: FlexDirection::Column,
                    padding: {UiRect::axes(px(18), px(16))},
                }
            ]
            --
            @footer()
        ]
    }
}

/// A recessed panel: the list well and the details card.
fn inset() -> impl Scene {
    bsn! {
        Node { border: px(1), border_radius: BorderRadius::all(px(6)) }
        BackgroundColor(ro_dark::INSET)
        BorderColor::all(ro_dark::SLOT_STROKE)
    }
}

/// The action row. The negative margin cancels the buttons' baked shadow so their
/// shapes line up with the panels above.
fn footer() -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(14),
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
        Children [
            Node { width: px(88.0 + 2.0 * BUTTON_SHADOW) }
            Children [
                @button(ButtonArt::NORMAL, "Back")
                on(back_to_login)
            ]
            --
            Node { flex_grow: 1.0 }
            --
            @chrome_text("Up / Down to choose · Enter to play".into(), 11.0, ro_dark::FAINT)
            --
            Node { min_width: px(168.0 + 2.0 * BUTTON_SHADOW) }
            Children [
                EnterButton
                @button(ButtonArt::PRIMARY, "Enter")
                on(|_: On<Activate>, mut commands: Commands| commands.trigger(EnterWorld))
            ]
        ]
    }
}

/// One world row: initial badge, name with type chips, and the status pill over a
/// population bar. A click selects it and a double-click enters it.
pub(super) fn server_row(index: usize, server: &ServerInfo) -> impl Scene + use<> {
    let status = status_of(server);
    let color = status_color(status);
    let down = status == ServerStatus::Offline;
    let ratio = server_fill(server);
    let initial = initial_of(&server.name);
    let initial_color = if down {
        ro_dark::FAINT
    } else {
        ro_dark::BRONZE_LIGHT
    };
    let (subtitle, subtitle_color) = if down {
        ("Closed for maintenance".to_string(), ro_dark::STATUS_BUSY)
    } else {
        let place = match server.region.as_str() {
            "" => pop_word(ratio),
            region => region,
        };
        (
            format!("{} · {place}", hero_count(server.heroes.len())),
            ro_dark::MUTED,
        )
    };
    let users = if down {
        "-".to_string()
    } else {
        thousands(server.users.into())
    };
    let fill = if down { 0.0 } else { ratio * 100.0 };
    let name = server.name.clone();
    let chips = chips(server);

    bsn! {
        ServerRow(index)
        Hovered
        Node {
            height: px(62),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::horizontal(px(12))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::SLOT)
        BorderColor::all(ro_dark::INSET_RULE)
        BoxShadow
        on(move |click: On<PointerClick>, mut selected: ResMut<SelectedServer>, mut commands: Commands| {
            selected.0 = index;
            if click.count >= 2 {
                commands.trigger(EnterWorld);
            }
        })
        Children [
            @initial_badge(initial, initial_color)
            --
            Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column }
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(6) }
                Children [
                    @chrome_text(name, 14.0, ro_dark::STRONG)
                    --
                    {chips}
                ]
                --
                @chrome_text(subtitle, 11.0, subtitle_color)
            ]
            --
            Node {
                width: px(104),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
            }
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(5) }
                Children [
                    Node { width: px(6), height: px(6), border_radius: BorderRadius::MAX }
                    BackgroundColor(color)
                    --
                    @chrome_text(status_label(status).into(), 11.0, color)
                    --
                    Node { flex_grow: 1.0 }
                    --
                    @chrome_text(users, 11.0, ro_dark::MUTED)
                ]
                --
                Node {
                    height: px(5),
                    border_radius: BorderRadius::all(px(3)),
                    overflow: {Overflow::clip()},
                }
                BackgroundColor(ro_dark::BAR_TROUGH)
                Children [
                    Node { width: percent(fill), height: percent(100) }
                    BackgroundColor(color)
                ]
            ]
        ]
    }
}

/// The upper-cased first letter of `name`, for a badge.
fn initial_of(name: &str) -> String {
    name.chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string()
}

/// The server's type chip, plus `NEW` for a freshly opened world.
fn chips(server: &ServerInfo) -> Vec<impl Scene + use<>> {
    let (label, color) = type_chip(server.server_type);
    let mut chips = vec![chip(label, color)];
    if server.new_server != 0 {
        chips.push(chip("NEW", ro_dark::STATUS_ONLINE));
    }
    chips
}

fn chip(label: &'static str, color: Color) -> impl Scene {
    bsn! {
        Node {
            height: px(17),
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(6))},
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor({color.with_alpha(0.16)})
        Children [ @chrome_text(label.into(), 10.0, color) ]
    }
}

/// The server's initial over a bronze diamond.
fn initial_badge(initial: String, color: Color) -> impl Scene {
    let tilt = diamond_tilt();
    bsn! {
        Node {
            width: px(36),
            height: px(36),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Children [
            Node {
                position_type: PositionType::Absolute,
                left: px(4),
                top: px(4),
                width: px(28),
                height: px(28),
                border: px(1),
            }
            tilt
            BackgroundColor(ro_dark::INSET)
            BorderColor::all(ro_dark::TAB_STROKE)
            --
            @display_text(initial, 15.0, color)
        ]
    }
}

fn diamond_tilt() -> UiTransform {
    UiTransform::from_rotation(Rot2::degrees(45.0))
}

/// Runtime text in the title font; `title_text` only takes static strings.
fn display_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_TITLE),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        @ignore_picking()
    }
}

/// The details card for the selected world: identity, description, notes, stats, the
/// account's heroes there and when it was last played. `jobs` resolves hero class ids
/// to display names and `now` is the server clock.
pub(super) fn details(
    server: &ServerInfo,
    jobs: Option<&JobSpriteRegistry>,
    now: Option<u64>,
) -> impl Scene + use<> {
    let status = status_of(server);
    let players = match status {
        ServerStatus::Offline => status_label(status).to_string(),
        _ => format!(
            "{} · {}",
            thousands(server.users.into()),
            status_label(status)
        ),
    };
    let name = server.name.clone();
    let chips = chips(server);
    let description: Vec<_> = (!server.description.is_empty())
        .then(|| chrome_text(server.description.clone(), 12.0, ro_dark::LABEL))
        .into_iter()
        .collect();
    let notes: Vec<_> = match status {
        ServerStatus::Offline => vec![note("Closed for maintenance")],
        ServerStatus::Full => vec![note("Full right now")],
        _ => Vec::new(),
    };
    let mut stats = vec![stat("PLAYERS", players, status_color(status))];
    if !server.region.is_empty() {
        stats.push(stat("REGION", server.region.clone(), ro_dark::STRONG));
    }
    if let Some(rates) = server.rates {
        stats.push(stat(
            "BASE · JOB · DROP",
            rates_label(rates),
            ro_dark::STRONG,
        ));
    }
    let heroes: Vec<_> = server
        .heroes
        .iter()
        .map(|hero| {
            let job = jobs
                .and_then(|jobs| jobs.get_display_name(hero.class.into()))
                .unwrap_or("Unknown");
            hero_row(hero, job.to_string())
        })
        .collect();
    let no_heroes: Vec<_> = server
        .heroes
        .is_empty()
        .then(no_heroes)
        .into_iter()
        .collect();
    let last_played: Vec<_> = last_played(&server.heroes, now)
        .map(|line| chrome_text(line, 11.0, ro_dark::MUTED))
        .into_iter()
        .collect();
    bsn! {
        Node {
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
        }
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(8) }
            Children [
                @display_text(name, 21.0, ro_dark::HERO)
                --
                {chips}
            ]
            --
            {description}
            --
            {notes}
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(6), row_gap: px(6) }
            Children [ {stats} ]
            --
            @chrome_text("YOUR HEROES HERE".into(), 10.0, ro_dark::ACCENT)
            --
            ScrollArea
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                overflow: {Overflow::scroll_y()},
            }
            Children [ {heroes} -- {no_heroes} ]
            --
            {last_played}
        ]
    }
}

/// One of the account's characters: initial, name, job and base level.
fn hero_row(hero: &HeroSummary, job: String) -> impl Scene + use<> {
    let initial = initial_of(&hero.name);
    let name = hero.name.clone();
    let level = format!("Lv {}", hero.base_level);
    bsn! {
        Node {
            height: px(34),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::horizontal(px(10))},
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::SLOT)
        Children [
            Node {
                width: px(22),
                height: px(22),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
                border_radius: BorderRadius::MAX,
            }
            BackgroundColor(ro_dark::TAB_ACTIVE)
            BorderColor::all(ro_dark::TAB_STROKE)
            Children [ @chrome_text(initial, 10.0, ro_dark::BRONZE_LIGHT) ]
            --
            @chrome_text(name, 13.0, ro_dark::STRONG)
            --
            @chrome_text(job, 11.0, ro_dark::MUTED)
            --
            Node { flex_grow: 1.0 }
            --
            @chrome_text(level, 11.0, ro_dark::ACCENT)
        ]
    }
}

fn no_heroes() -> impl Scene {
    bsn! {
        Node {
            height: px(34),
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BorderColor::all(ro_dark::SLOT_STROKE)
        Children [ @chrome_text("No heroes yet".into(), 12.0, ro_dark::MUTED) ]
    }
}

fn note(text: &'static str) -> impl Scene {
    bsn! {
        Node {
            padding: {UiRect::axes(px(10), px(7))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor({ro_dark::STATUS_BUSY.with_alpha(0.1)})
        BorderColor::all(ro_dark::STATUS_BUSY.with_alpha(0.45))
        Children [ @chrome_text(text.into(), 12.0, ro_dark::BRONZE_LIGHT) ]
    }
}

/// One cell of the stats grid: two cells share a row and an odd last one fills it.
fn stat(key: &'static str, value: String, color: Color) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: percent(40),
            flex_direction: FlexDirection::Column,
            padding: {UiRect::axes(px(9), px(6))},
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::SCROLL_TRACK)
        Children [
            @chrome_text(key.into(), 10.0, ro_dark::FAINT)
            --
            @chrome_text(value, 13.0, color)
        ]
    }
}

/// The wait modal: a click-eating backdrop under a small titled window.
pub(super) fn connecting_modal(world: String) -> impl Scene {
    let tilt = diamond_tilt();
    let line = format!("Reaching {world}");
    let modal = ConnectingModal {
        world,
        timer: Timer::from_seconds(CONNECT_TIMEOUT_SECS, TimerMode::Once),
    };
    bsn! {
        modal
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({Color::srgba_u8(5, 7, 10, 158)})
        GlobalZIndex(1)
        Children [
            @window_titled()
            Node {
                width: px(MODAL_WIDTH + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
            }
            Children [
                @window_title("Entering World")
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: {UiRect::vertical(px(10))},
                }
                Children [
                    Node { width: px(8), height: px(8) }
                    tilt
                    BackgroundColor(ro_dark::FROST_GLOW)
                    --
                    @chrome_text(line, 13.0, ro_dark::STRONG)
                ]
            ]
        ]
    }
}
