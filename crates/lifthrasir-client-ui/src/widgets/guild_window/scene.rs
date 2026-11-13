//! The guild window scenes: the frame [`window`] spawns once, the page skeleton of the
//! tab on show, and the regions `rebuild_guild_ui` projects from their view-models.
//! Sizes follow the `Guild` artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{Button, ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, HoverColors, INPUT_SHADOW, WINDOW_SHADOW, close_button, display_text, inset,
    segment, tab, tab_text, window_title_bar_led, window_titled,
};

use super::view::{
    ALLY_LIMIT, ANTAGONIST_LIMIT, ActivityLine, FooterView, MemberRowView, NoticeView, PageView,
    PositionEditView, PositionRowView, PositionsView, RelationCardView, RelationsView,
    SelectionView, SkillCardView, SummaryView, TabView, ToastView, ToolsView,
};
use super::{
    EmblemSlot, ExpelReasonField, GuildAction, GuildAllianceNameField, GuildAntagonistNameField,
    GuildHost, GuildInviteNameField, GuildTitlebar, GuildWindowRoot, MemberSearchField, MemberSort,
    NoticeBodyField, NoticeSubjectField, Perm, PositionNameField, PositionTaxField, SkillCell,
    on_guild_button, on_skill_cell_click,
};
use crate::screens::shared::{
    DANGER_TINT, colored_lines, number_field, overline, row_button, search_field, text_field,
    usable, wide_button,
};

const WINDOW_LEFT: f32 = 110.0;
const WINDOW_TOP: f32 = 10.0;
/// Width of the window's shape.
const WINDOW_WIDTH: f32 = 1004.0;
const PANEL_HEIGHT: f32 = 512.0;
/// Width of a guild skill card: two share the panel beside the scrollbar.
const SKILL_CARD_WIDTH: f32 = 450.0;
/// Width of a permission column of the positions table.
const PERM_COLUMN: f32 = 92.0;
/// How far the toast floats over the window's bottom edge.
const TOAST_LIFT: f32 = 70.0;

/// The roster's columns: what they sort by, their caption, their width (the name
/// takes what is left) and whether they are right-aligned.
const MEMBER_COLUMNS: [(MemberSort, &str, Option<f32>, bool); 6] = [
    (MemberSort::Name, "NAME", None, false),
    (MemberSort::Job, "CLASS", Some(130.0), false),
    (MemberSort::Level, "LV", Some(44.0), true),
    (MemberSort::Position, "POSITION", Some(110.0), false),
    (MemberSort::Exp, "EXP GIVEN", Some(110.0), true),
    (MemberSort::Seen, "LAST SEEN", Some(150.0), true),
];

/// The frame: the title bar, the tab strip, the panel and the footer.
pub fn window() -> impl Scene {
    bsn! {
        GuildWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Visibility::Hidden
        Pickable
        Children [
            GuildTitlebar
            @window_title_bar_led(
                "Guild",
                bsn! {
                    GuildHost::Title
                    Node { align_items: AlignItems::Center }
                    @ignore_picking()
                },
                bsn! {
                    @close_button()
                    GuildAction::CloseWindow
                    on(on_guild_button)
                },
            )
            Pickable
            on(drag_window::<GuildTitlebar, GuildWindowRoot>)
            --
            GuildHost::Tabs
            Node { margin: {UiRect::top(px(10))} }
            @ignore_picking()
            --
            GuildHost::Page
            Node {
                height: px(PANEL_HEIGHT),
                padding: px(16),
                border: px(1),
                border_radius: {BorderRadius::bottom(px(6))},
            }
            BackgroundColor(ro_dark::INSET)
            BorderColor::all(ro_dark::SLOT_STROKE)
            @ignore_picking()
            --
            GuildHost::Footer
            Node {
                height: px(32),
                margin: {UiRect::new(px(0), px(0), px(14), px(8))},
            }
            @ignore_picking()
            --
            GuildHost::Toast
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(WINDOW_SHADOW + 16.0 + TOAST_LIFT),
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
        ]
    }
}

pub(super) fn title(line: String) -> impl Scene {
    chrome_text(line, 12.0, ro_dark::LABEL)
}

pub(super) fn tabs(tabs: Vec<TabView>) -> impl Scene {
    let tabs: Vec<_> = tabs.into_iter().map(tab_button).collect();
    bsn! {
        Node { flex_grow: 1.0, column_gap: px(4) }
        @ignore_picking()
        Children [ {tabs} ]
    }
}

fn tab_button(view: TabView) -> impl Scene {
    let (label, count) = tab_text(view.on);
    let pick = GuildAction::Tab(view.tab);
    bsn! {
        Button
        @tab(view.on)
        pick
        on(on_guild_button)
        Children [
            @chrome_text(view.label.into(), 13.0, label)
            --
            @chrome_text(view.note, 11.0, count)
        ]
    }
}

/// A raised card on the panel.
fn card() -> impl Scene {
    bsn! {
        Node {
            padding: {UiRect::axes(px(14), px(12))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::SLOT)
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
    }
}

/// A space that pushes what follows to the far end of its row.
fn spacer() -> impl Scene {
    bsn! {
        Node { flex_grow: 1.0 }
        @ignore_picking()
    }
}

/// A region that scrolls the content of `host`, with the scrollbar on its right edge.
fn scroll(host: GuildHost) -> impl Scene {
    bsn! {
        Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
        @ignore_picking()
        Children [
            #list
            host
            ScrollArea
            Pickable
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                right: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                padding: {UiRect::right(px(12))},
                overflow: {Overflow::scroll_y()},
            }
            --
            @FeathersScrollbar { @target: #list, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(3),
                top: px(4),
                bottom: px(4),
                width: px(6),
            }
        ]
    }
}

/// A status dot: lit green while online.
fn dot(online: bool) -> impl Scene {
    let fill = if online {
        ro_dark::JOB_EXP
    } else {
        ro_dark::SLOT_STROKE
    };
    bsn! {
        Node { width: px(8), height: px(8), flex_shrink: 0.0, border_radius: BorderRadius::MAX }
        BackgroundColor(fill)
        @ignore_picking()
    }
}

/// The chevron glyph turned to point down or up.
fn caret(down: bool, color: Color) -> impl Scene {
    let turn = UiTransform::from_rotation(Rot2::degrees(if down { 90.0 } else { -90.0 }));
    bsn! {
        @glyph_icon("chev", 9.0, color)
        turn
    }
}

/// A guild's emblem `size` px square. The plate behind it shows until
/// `sync_emblems` has the image.
pub(super) fn emblem(guild_id: u32, emblem_id: u32, size: f32) -> impl Scene {
    bsn! {
        Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(ro_dark::WINDOW_BODY)
        BorderColor::all(ro_dark::BRONZE)
        @ignore_picking()
        Children [
            EmblemSlot { guild_id, emblem_id }
            ImageNode
            Node { width: percent(100), height: percent(100) }
            Visibility::Hidden
            @ignore_picking()
        ]
    }
}

/// A progress bar filled to `fill` percent.
pub(super) fn bar(fill: f32, color: Color) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            height: px(10),
            padding: px(1),
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::BAR_TROUGH)
        BorderColor::all(ro_dark::BAR_FRAME)
        @ignore_picking()
        Children [
            Node {
                width: {Val::Percent(fill)},
                height: percent(100),
                border_radius: BorderRadius::all(px(3)),
            }
            BackgroundColor(color)
            @ignore_picking()
        ]
    }
}

/// The skeleton of the tab on show; its hosts are filled like the frame's.
pub(super) fn page(view: PageView) -> Box<dyn Scene> {
    match view {
        PageView::Overview => Box::new(overview_page()),
        PageView::Members { can_invite } => Box::new(members_page(can_invite)),
        PageView::Positions { can_edit } => Box::new(positions_page(can_edit)),
        PageView::Skills => Box::new(skills_page()),
        PageView::Relations { can_manage } => Box::new(relations_page(can_manage)),
    }
}

/// Fills its host both ways and stacks its children.
fn page_column() -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
        }
        @ignore_picking()
    }
}

fn overview_page() -> impl Scene {
    bsn! {
        Node { flex_grow: 1.0, min_width: px(0), column_gap: px(16) }
        @ignore_picking()
        Children [
            GuildHost::Summary
            Node { width: px(340), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
            @ignore_picking()
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
            }
            @ignore_picking()
            Children [
                GuildHost::Notice
                Node { flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                --
                @card()
                Node {
                    flex_grow: 1.0,
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                }
                Children [
                    @overline("RECENT ACTIVITY".into(), ro_dark::ACCENT)
                    --
                    @scroll(GuildHost::Activity)
                ]
            ]
        ]
    }
}

pub(super) fn summary(view: SummaryView) -> impl Scene {
    let stats: Vec<_> = view.stats.into_iter().map(stat).collect();
    let sub = vec![vec![
        (
            ro_dark::MUTED,
            format!("Guild Lv {} \u{b7} Master ", view.level),
        ),
        (ro_dark::BRONZE_LIGHT, view.master),
    ]];
    let upload = view.can_upload.then(|| {
        EntityScene(bsn! {
            @row_button(ButtonArt::NORMAL, "Change Emblem", 110.0)
            Node { align_self: AlignSelf::FlexStart }
            GuildAction::PickEmblem
            on(on_guild_button)
        })
    });
    bsn! {
        Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(12) }
        @ignore_picking()
        Children [
            @card()
            Node { align_items: AlignItems::Center, column_gap: px(14) }
            Children [
                @emblem(view.guild_id, view.emblem_id, 64.0)
                --
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                }
                @ignore_picking()
                Children [
                    @display_text(view.name, 19.0, ro_dark::HERO)
                    --
                    @colored_lines(sub)
                    --
                    Node { align_items: AlignItems::Center, column_gap: px(8) }
                    @ignore_picking()
                    Children [
                        @bar(view.exp, ro_dark::BASE_EXP)
                        --
                        @chrome_text(view.exp_label, 11.0, ro_dark::LABEL)
                    ]
                ]
            ]
            --
            @card()
            Node { flex_wrap: FlexWrap::Wrap, row_gap: px(10) }
            Children [ {stats} ]
            --
            {upload}
        ]
    }
}

fn stat((label, value, tone): (&'static str, String, Color)) -> impl Scene {
    bsn! {
        Node { width: percent(50), flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 11.0, ro_dark::MUTED)
            --
            @chrome_text(value, 15.0, tone)
            TextLayout { linebreak: LineBreak::NoWrap }
        ]
    }
}

/// The guild notice, or the two fields that replace it.
pub(super) fn notice(view: NoticeView) -> Box<dyn Scene> {
    if view.editing {
        return Box::new(notice_editor(view));
    }
    let subject = if view.subject.is_empty() && view.body.is_empty() {
        "No guild notice has been posted.".to_string()
    } else {
        view.subject
    };
    let body =
        (!view.body.is_empty()).then(|| EntityScene(chrome_text(view.body, 13.0, ro_dark::BODY)));
    let by = (!view.by.is_empty()).then(|| EntityScene(chrome_text(view.by, 11.0, ro_dark::MUTED)));
    Box::new(bsn! {
        @card()
        Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
        Children [
            Node { align_items: AlignItems::Center }
            @ignore_picking()
            Children [
                @overline("GUILD NOTICE".into(), ro_dark::ACCENT)
                --
                @spacer()
                --
                @row_button(ButtonArt::NORMAL, "Edit Notice", 84.0)
                @usable(view.can_edit, Color::WHITE)
                GuildAction::EditNotice
                on(on_guild_button)
            ]
            --
            @chrome_text(subject, 15.0, ro_dark::HERO)
            --
            {body}
            --
            {by}
        ]
    })
}

fn notice_editor(view: NoticeView) -> impl Scene {
    bsn! {
        @card()
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(8) }
            @ignore_picking()
            Children [
                @overline("GUILD NOTICE".into(), ro_dark::ACCENT)
                --
                @spacer()
                --
                @row_button(ButtonArt::NORMAL, "Cancel", 60.0)
                GuildAction::CancelNotice
                on(on_guild_button)
                --
                @row_button(ButtonArt::PRIMARY, "Save", 60.0)
                GuildAction::SaveNotice
                on(on_guild_button)
            ]
            --
            NoticeSubjectField
            @text_field(view.subject, 60)
            --
            NoticeBodyField
            @text_field(view.body, 120)
        ]
    }
}

pub(super) fn activity(lines: Vec<ActivityLine>) -> Box<dyn Scene> {
    if lines.is_empty() {
        return Box::new(chrome_text(
            "Nothing has happened yet.".into(),
            12.0,
            ro_dark::MUTED,
        ));
    }
    let lines: Vec<_> = lines.into_iter().map(activity_line).collect();
    Box::new(bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [ {lines} ]
    })
}

fn activity_line(line: ActivityLine) -> impl Scene {
    let tilt = UiTransform::from_rotation(Rot2::degrees(45.0));
    let tone = line.tone;
    bsn! {
        Node {
            flex_shrink: 0.0,
            column_gap: px(10),
            padding: {UiRect::vertical(px(6))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node { width: px(7), height: px(7), flex_shrink: 0.0, margin: {UiRect::top(px(5))} }
            tilt
            BackgroundColor(tone)
            @ignore_picking()
            --
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            @ignore_picking()
            Children [ @chrome_text(line.text, 12.0, ro_dark::BODY) ]
            --
            @chrome_text(line.when, 12.0, ro_dark::MUTED)
            TextLayout { linebreak: LineBreak::NoWrap }
        ]
    }
}

fn members_page(can_invite: bool) -> impl Scene {
    let hint = if can_invite {
        "Your position can invite. Changing positions is for the guild master."
    } else {
        "Your position can\u{2019}t invite. Changing positions is for the guild master."
    };
    bsn! {
        @page_column()
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(8) }
            @ignore_picking()
            Children [
                @search_field::<MemberSearchField>("Search name, class or map")
                Node {
                    width: px(240.0 + 2.0 * INPUT_SHADOW),
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                }
                --
                GuildHost::Tools
                Node { align_items: AlignItems::Center }
                @ignore_picking()
                --
                @spacer()
                --
                @search_field::<GuildInviteNameField>("Hero name")
                Node {
                    width: px(150.0 + 2.0 * INPUT_SHADOW),
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                }
                --
                @row_button(ButtonArt::PRIMARY, "Invite", 64.0)
                @usable(can_invite, Color::WHITE)
                GuildAction::Invite
                on(on_guild_button)
            ]
            --
            @inset()
            BackgroundColor(ro_dark::SCROLL_TRACK)
            Node { flex_grow: 1.0, min_height: px(0), flex_direction: FlexDirection::Column }
            @ignore_picking()
            Children [
                GuildHost::MemberHead
                Node {
                    height: px(30),
                    flex_shrink: 0.0,
                    border: {UiRect::bottom(px(1))},
                }
                BorderColor::all(ro_dark::INSET_RULE)
                @ignore_picking()
                --
                @scroll(GuildHost::MemberRows)
            ]
            --
            @inset()
            GuildHost::Selection
            Node {
                height: px(52),
                flex_shrink: 0.0,
                padding: {UiRect::horizontal(px(12))},
                align_items: AlignItems::Center,
            }
            @ignore_picking()
            --
            @chrome_text(hint.into(), 11.0, ro_dark::MUTED)
        ]
    }
}

pub(super) fn tools(view: ToolsView) -> impl Scene {
    let (ink, _) = tab_text(view.online_only);
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            Button
            @segment(view.online_only)
            Node { height: px(26), border_radius: BorderRadius::all(px(13)) }
            GuildAction::ToggleOnline
            on(on_guild_button)
            Children [
                @dot(true)
                --
                @chrome_text("Online only".into(), 12.0, ink)
            ]
            --
            @chrome_text(view.count, 12.0, ro_dark::MUTED)
        ]
    }
}

/// A fixed-width cell of a table row; without a width it takes what is left.
fn cell(width: Option<f32>, right: bool) -> impl Scene {
    let justify = if right {
        JustifyContent::FlexEnd
    } else {
        JustifyContent::FlexStart
    };
    let (width, basis, grow): (Val, Val, f32) = match width {
        Some(width) => (px(width), auto(), 0.0),
        None => (auto(), px(0), 1.0),
    };
    bsn! {
        Node {
            width: width,
            flex_grow: grow,
            flex_shrink: grow,
            flex_basis: basis,
            min_width: px(0),
            align_items: AlignItems::Center,
            justify_content: justify,
            column_gap: px(6),
            overflow: {Overflow::clip()},
        }
    }
}

/// The roster's column captions; a click sorts by the column, a second one flips it.
pub(super) fn member_head((sort, reversed): (MemberSort, bool)) -> impl Scene {
    let columns: Vec<_> = MEMBER_COLUMNS
        .into_iter()
        .map(|(by, label, width, right)| {
            let arrow = (by == sort).then(|| EntityScene(caret(!reversed, ro_dark::FROST)));
            bsn! {
                Button
                Hovered
                @cell(width, right)
                Pickable
                GuildAction::Sort(by)
                on(on_guild_button)
                Children [
                    @overline(label.into(), ro_dark::ACCENT)
                    --
                    {arrow}
                ]
            }
        })
        .collect();
    bsn! {
        Node {
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::new(px(28), px(22), px(0), px(0))},
        }
        @ignore_picking()
        Children [ {columns} ]
    }
}

pub(super) fn member_rows(rows: Vec<MemberRowView>) -> Box<dyn Scene> {
    if rows.is_empty() {
        return Box::new(bsn! {
            Node { flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center }
            @ignore_picking()
            Children [ @chrome_text("No members match".into(), 12.0, ro_dark::MUTED) ]
        });
    }
    let rows: Vec<_> = rows.into_iter().map(member_row).collect();
    Box::new(bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [ {rows} ]
    })
}

fn member_row(view: MemberRowView) -> impl Scene {
    let (fill, stroke) = if view.selected {
        (
            ro_dark::FROST_GLOW.with_alpha(0.08),
            ro_dark::SELECT_RING.with_alpha(0.6),
        )
    } else {
        (Color::NONE, Color::NONE)
    };
    let colors = HoverColors {
        rest: (fill, stroke),
        hover: if view.selected {
            (fill, stroke)
        } else {
            (Color::WHITE.with_alpha(0.03), Color::NONE)
        },
    };
    let ink = match (view.me, view.online) {
        (true, _) => ro_dark::FROST,
        (false, true) => ro_dark::STRONG,
        (false, false) => ro_dark::MUTED,
    };
    let seen_tone = if view.online {
        ro_dark::STATUS_ONLINE
    } else {
        ro_dark::MUTED
    };
    let you = view
        .me
        .then(|| EntityScene(chrome_text("You".into(), 11.0, ro_dark::FROST)));
    let [name, job, level, position, exp, seen] =
        MEMBER_COLUMNS.map(|(_, _, width, right)| cell(width, right));
    let pick = GuildAction::Select(view.char_id);
    bsn! {
        Button
        Hovered
        colors
        pick
        Node {
            height: px(40),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::horizontal(px(9))},
            border: px(1),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        on(on_guild_button)
        Children [
            @dot(view.online)
            --
            @name
            @ignore_picking()
            Children [
                @chrome_text(view.name, 13.0, ink)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                {you}
            ]
            --
            @job
            @ignore_picking()
            Children [
                @chrome_text(view.job, 12.0, ro_dark::LABEL)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
            --
            @level
            @ignore_picking()
            Children [ @chrome_text(view.level.to_string(), 12.0, ro_dark::STRONG) ]
            --
            @position
            @ignore_picking()
            Children [
                @chrome_text(view.position, 12.0, view.position_tone)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
            --
            @exp
            @ignore_picking()
            Children [ @chrome_text(view.exp, 12.0, ro_dark::BRONZE_LIGHT) ]
            --
            @seen
            @ignore_picking()
            Children [
                @chrome_text(view.seen, 12.0, seen_tone)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
            --
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(-1),
                height: px(1),
            }
            BackgroundColor(ro_dark::INSET_RULE)
            @ignore_picking()
        ]
    }
}

/// The selected member with what can be done to them, or the expel prompt.
pub(super) fn selection(view: Option<SelectionView>) -> impl Scene {
    let parts = selection_parts(view);
    bsn! {
        Node { flex_grow: 1.0, min_width: px(0), align_items: AlignItems::Center, column_gap: px(10) }
        @ignore_picking()
        Children [ {parts} ]
    }
}

fn selection_parts(view: Option<SelectionView>) -> Vec<Box<dyn Scene>> {
    let Some(view) = view else {
        return vec![Box::new(bsn! {
            Node { flex_grow: 1.0, justify_content: JustifyContent::Center }
            @ignore_picking()
            Children [ @chrome_text("Select a member".into(), 12.0, ro_dark::MUTED) ]
        })];
    };
    let char_id = view.char_id;
    let mut bar: Vec<Box<dyn Scene>> = vec![
        Box::new(dot(view.online)),
        Box::new(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(view.title, 13.0, ro_dark::STRONG)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @chrome_text(view.meta, 11.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
        }),
    ];
    if view.ask_expel {
        bar.push(Box::new(chrome_text(
            format!("Expel {}?", view.name),
            12.0,
            ro_dark::DANGER_GLOW,
        )));
        bar.push(Box::new(bsn! {
            @search_field::<ExpelReasonField>("Reason")
            Node {
                width: px(190.0 + 2.0 * INPUT_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }
        }));
        bar.push(Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Keep", 48.0)
            GuildAction::KeepMember
            on(on_guild_button)
        }));
        bar.push(Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Expel", 52.0)
            @usable(true, DANGER_TINT)
            GuildAction::Expel(char_id)
            on(on_guild_button)
        }));
        return bar;
    }
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Whisper", 64.0)
        @usable(view.can_whisper, Color::WHITE)
        GuildAction::Whisper(char_id)
        on(on_guild_button)
    }));
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Invite to Party", 104.0)
        @usable(view.can_party, Color::WHITE)
        GuildAction::PartyInvite(char_id)
        on(on_guild_button)
    }));
    bar.push(Box::new(position_control(
        char_id,
        view.position,
        view.can_assign,
        view.menu,
    )));
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Expel", 52.0)
        @usable(view.can_expel, DANGER_TINT)
        GuildAction::AskExpel
        on(on_guild_button)
    }));
    bar
}

/// The member's position as a button that opens `menu` over itself.
fn position_control(
    char_id: u32,
    position: String,
    can_assign: bool,
    menu: Vec<(u32, String, bool)>,
) -> impl Scene {
    let options: Vec<_> = menu
        .into_iter()
        .map(|(index, title, held)| {
            let (ink, _) = tab_text(held);
            bsn! {
                Button
                @segment(held)
                Node { flex_shrink: 0.0, justify_content: JustifyContent::FlexStart }
                GuildAction::Assign(char_id, index)
                on(on_guild_button)
                Children [ @chrome_text(title, 12.0, ink) ]
            }
        })
        .collect();
    let menu = (!options.is_empty()).then(|| {
        EntityScene(bsn! {
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                bottom: px(30),
                width: px(170),
                max_height: px(260),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                padding: px(4),
                border: px(1),
                border_radius: BorderRadius::all(px(6)),
                overflow: {Overflow::scroll_y()},
            }
            BackgroundColor(ro_dark::TOOLTIP_BODY)
            BorderColor::all(ro_dark::BUTTON_STROKE)
            Pickable
            Children [ {options} ]
        })
    });
    bsn! {
        Node { position_type: PositionType::Relative, flex_shrink: 0.0 }
        @ignore_picking()
        Children [
            @row_button(ButtonArt::NORMAL, position, 130.0)
            @usable(can_assign, Color::WHITE)
            GuildAction::TogglePositionMenu
            on(on_guild_button)
            --
            {menu}
        ]
    }
}

fn positions_page(can_edit: bool) -> impl Scene {
    let note = if can_edit {
        "Click a mark to change it \u{b7} Click a name to rename it or set its tax"
    } else {
        "Only the guild master can edit"
    };
    let heads: Vec<_> = Perm::ALL
        .into_iter()
        .map(|perm| {
            bsn! {
                @cell(Some(PERM_COLUMN), false)
                Node { justify_content: JustifyContent::Center }
                @ignore_picking()
                Children [ @overline(perm.label().into(), ro_dark::ACCENT) ]
            }
        })
        .collect();
    bsn! {
        @page_column()
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(6) }
            @ignore_picking()
            Children [
                @chrome_text(
                    "What each position can do, and how much of their EXP goes to the guild.".into(),
                    12.0,
                    ro_dark::MUTED,
                )
                --
                @spacer()
                --
                @glyph_icon("lock", 11.0, ro_dark::MUTED)
                --
                @chrome_text(note.into(), 11.0, ro_dark::MUTED)
            ]
            --
            @inset()
            BackgroundColor(ro_dark::SCROLL_TRACK)
            Node { flex_grow: 1.0, min_height: px(0), flex_direction: FlexDirection::Column }
            @ignore_picking()
            Children [
                Node {
                    height: px(34),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: {UiRect::new(px(14), px(26), px(0), px(0))},
                    border: {UiRect::bottom(px(1))},
                }
                BorderColor::all(ro_dark::INSET_RULE)
                @ignore_picking()
                Children [
                    @cell(None, false)
                    @ignore_picking()
                    Children [ @overline("POSITION".into(), ro_dark::ACCENT) ]
                    --
                    @cell(Some(80.0), false)
                    Node { justify_content: JustifyContent::Center }
                    @ignore_picking()
                    Children [ @overline("MEMBERS".into(), ro_dark::ACCENT) ]
                    --
                    {heads}
                    --
                    @cell(Some(80.0), true)
                    @ignore_picking()
                    Children [ @overline("EXP TAX".into(), ro_dark::ACCENT) ]
                ]
                --
                @scroll(GuildHost::Positions)
            ]
            --
            GuildHost::PositionEdit
            Node { flex_shrink: 0.0 }
            @ignore_picking()
        ]
    }
}

pub(super) fn positions(view: PositionsView) -> impl Scene {
    let can_edit = view.can_edit;
    let rows: Vec<_> = view
        .rows
        .into_iter()
        .map(|row| position_row(row, can_edit))
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [ {rows} ]
    }
}

fn position_row(view: PositionRowView, can_edit: bool) -> impl Scene {
    let editable = can_edit && !view.protected;
    let index = view.index;
    let fill = match (view.selected, view.mine) {
        (true, _) => ro_dark::FROST_GLOW.with_alpha(0.1),
        (false, true) => ro_dark::FROST_GLOW.with_alpha(0.06),
        (false, false) => Color::NONE,
    };
    let mine = view
        .mine
        .then(|| EntityScene(chrome_text("Your position".into(), 11.0, ro_dark::FROST)));
    // Only the master's editable rows take the click that opens the edit bar.
    let pick = editable.then(|| {
        bsn! {
            Button
            Hovered
            Pickable
            GuildAction::SelectPosition(index)
            on(on_guild_button)
        }
    });
    let still = (!editable).then(ignore_picking);
    let marks: Vec<_> = Perm::ALL
        .into_iter()
        .zip(view.perms)
        .map(|(perm, granted)| perm_mark(index, perm, granted, editable))
        .collect();
    bsn! {
        Node {
            height: px(50),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::horizontal(px(14))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor(fill)
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @cell(None, false)
            Node { height: percent(100), column_gap: px(8) }
            @{pick}
            @{still}
            Children [
                @chrome_text(view.name, 13.0, view.tone)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                {mine}
            ]
            --
            @cell(Some(80.0), false)
            Node { justify_content: JustifyContent::Center }
            @ignore_picking()
            Children [ @chrome_text(view.count.to_string(), 12.0, ro_dark::STRONG) ]
            --
            {marks}
            --
            @cell(Some(80.0), true)
            @ignore_picking()
            Children [ @chrome_text(format!("{}%", view.tax), 13.0, ro_dark::BRONZE_LIGHT) ]
        ]
    }
}

/// One permission of a position: a box, ticked while granted. The master's editable
/// ones flip on a click.
fn perm_mark(index: u32, perm: Perm, granted: bool, editable: bool) -> impl Scene {
    let check = granted.then(|| EntityScene(glyph_icon("check", 10.0, ro_dark::CHECK_MARK)));
    let fill = if granted {
        ro_dark::SLOT
    } else {
        ro_dark::SCROLL_TRACK
    };
    let colors = HoverColors {
        rest: (fill, ro_dark::BUTTON_STROKE),
        hover: if editable {
            (ro_dark::SLOT_HOVER, ro_dark::BRONZE_HIGHLIGHT)
        } else {
            (fill, ro_dark::BUTTON_STROKE)
        },
    };
    let pick = editable.then(|| {
        bsn! {
            Button
            Pickable
            GuildAction::TogglePerm(index, perm)
            on(on_guild_button)
        }
    });
    let still = (!editable).then(ignore_picking);
    bsn! {
        Node { width: px(PERM_COLUMN), flex_shrink: 0.0, justify_content: JustifyContent::Center }
        @ignore_picking()
        Children [
            Hovered
            colors
            Node {
                width: px(16),
                height: px(16),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
            }
            BackgroundColor(fill)
            BorderColor::all(ro_dark::BUTTON_STROKE)
            @{pick}
            @{still}
            Children [ {check} ]
        ]
    }
}

/// The bar that renames the selected position and sets its tax.
pub(super) fn position_edit(view: PositionEditView) -> impl Scene {
    let save = GuildAction::SavePosition(view.index);
    bsn! {
        @inset()
        Node {
            flex_grow: 1.0,
            height: px(52),
            padding: {UiRect::horizontal(px(12))},
            align_items: AlignItems::Center,
            column_gap: px(10),
        }
        @ignore_picking()
        Children [
            @chrome_text(format!("Position {}", view.index + 1), 12.0, ro_dark::MUTED)
            --
            PositionNameField
            @text_field(view.name, 24)
            Node { width: px(220) }
            --
            @chrome_text("EXP tax".into(), 12.0, ro_dark::MUTED)
            --
            PositionTaxField
            @number_field(view.tax.to_string(), 3)
            Node { width: px(44) }
            --
            @chrome_text("%".into(), 12.0, ro_dark::MUTED)
            --
            @spacer()
            --
            @row_button(ButtonArt::PRIMARY, "Save", 64.0)
            save
            on(on_guild_button)
        ]
    }
}

fn skills_page() -> impl Scene {
    bsn! {
        @page_column()
        Children [
            GuildHost::SkillHead
            Node { flex_shrink: 0.0 }
            @ignore_picking()
            --
            @scroll(GuildHost::Skills)
        ]
    }
}

pub(super) fn skill_head(points: u32) -> impl Scene {
    bsn! {
        Node { flex_grow: 1.0, align_items: AlignItems::Center, column_gap: px(10) }
        @ignore_picking()
        Children [
            @chrome_text("Guild skill points".into(), 12.0, ro_dark::MUTED)
            --
            @chrome_text(points.to_string(), 16.0, ro_dark::BRONZE_LIGHT)
            --
            @chrome_text("\u{b7} Spent by the guild master".into(), 11.0, ro_dark::MUTED)
            --
            @spacer()
            --
            @chrome_text(
                "Right-click a skill for its details".into(),
                11.0,
                ro_dark::MUTED,
            )
        ]
    }
}

pub(super) fn skills(cards: Vec<SkillCardView>) -> impl Scene {
    let cards: Vec<_> = cards.into_iter().map(skill_card).collect();
    bsn! {
        Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8), row_gap: px(8) }
        @ignore_picking()
        Children [ {cards} ]
    }
}

fn skill_card(view: SkillCardView) -> impl Scene {
    let (ring, level_tone, tint) = if view.learned {
        (ro_dark::SLOT_STROKE, ro_dark::LABEL, Color::WHITE)
    } else {
        (
            ro_dark::INSET_RULE,
            ro_dark::MUTED,
            Color::WHITE.with_alpha(0.45),
        )
    };
    let name_tone = if view.learned {
        ro_dark::STRONG
    } else {
        ro_dark::LABEL
    };
    let icon = view.icon.map(|path| {
        EntityScene(bsn! {
            ImageNode { image: {path}, color: tint }
            Node { width: px(24), height: px(24) }
            @ignore_picking()
        })
    });
    let state = view
        .state
        .map(|(label, tone)| EntityScene(chrome_text(label.into(), 11.0, tone)));
    let skill_id = view.skill_id;
    let upgrade = view.can_upgrade.then(|| {
        EntityScene(bsn! {
            @row_button(ButtonArt::PRIMARY, "+", 24.0)
            GuildAction::SkillUp(skill_id)
            on(on_guild_button)
        })
    });
    bsn! {
        SkillCell(skill_id)
        Node {
            width: px(SKILL_CARD_WIDTH),
            min_height: px(62),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::axes(px(12), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::SLOT)
        BorderColor::all(ro_dark::INSET_RULE)
        Pickable
        on(on_skill_cell_click)
        Children [
            Node {
                width: px(40),
                height: px(40),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
            }
            BackgroundColor(ro_dark::INSET)
            BorderColor::all(ring)
            @ignore_picking()
            Children [ {icon} ]
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
            }
            @ignore_picking()
            Children [
                Node { align_items: AlignItems::Baseline, column_gap: px(8) }
                @ignore_picking()
                Children [
                    @chrome_text(view.name, 13.0, name_tone)
                    --
                    @chrome_text(view.level, 11.0, level_tone)
                ]
                --
                @chrome_text(view.blurb, 11.0, ro_dark::MUTED)
            ]
            --
            {state}
            --
            {upgrade}
        ]
    }
}

fn relations_page(can_manage: bool) -> impl Scene {
    bsn! {
        @page_column()
        Children [
            GuildHost::Relations
            Node { flex_grow: 1.0, min_height: px(0) }
            @ignore_picking()
            --
            Node { flex_shrink: 0.0, column_gap: px(16) }
            @ignore_picking()
            Children [
                @relation_action::<GuildAllianceNameField>(
                    can_manage,
                    "Their guild master\u{2019}s name",
                    "Propose Alliance",
                    GuildAction::ProposeAlliance,
                )
                --
                @relation_action::<GuildAntagonistNameField>(
                    can_manage,
                    "A hero of their guild",
                    "Declare Antagonist",
                    GuildAction::DeclareAntagonist,
                )
            ]
        ]
    }
}

/// The name field and button that start a relation; just the disabled button for
/// those whose position may not.
fn relation_action<F>(
    can_manage: bool,
    hint: &'static str,
    label: &'static str,
    action: GuildAction,
) -> impl Scene
where
    F: Component + Default + Clone + Unpin,
{
    let field = can_manage.then(|| {
        EntityScene(bsn! {
            @search_field::<F>(hint)
            Node {
                width: px(190.0 + 2.0 * INPUT_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }
        })
    });
    let note = if can_manage {
        "Can\u{2019}t change during a siege"
    } else {
        "Needs the Relations permission \u{b7} Can\u{2019}t change during a siege"
    };
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            {field}
            --
            @row_button(ButtonArt::NORMAL, label, 128.0)
            @usable(can_manage, Color::WHITE)
            action
            on(on_guild_button)
            --
            @chrome_text(note.into(), 11.0, ro_dark::MUTED)
        ]
    }
}

pub(super) fn relations(view: RelationsView) -> impl Scene {
    let can_manage = view.can_manage;
    bsn! {
        Node { flex_grow: 1.0, min_width: px(0), column_gap: px(16) }
        @ignore_picking()
        Children [
            @relation_column(
                ("ALLIES", ro_dark::FROST, ALLY_LIMIT),
                "Allies can\u{2019}t hit each other in sieges",
                view.allies,
                can_manage.then_some(("End Alliance", 96.0, GuildAction::BreakAlliance as fn(u32) -> GuildAction)),
            )
            --
            @relation_column(
                ("ANTAGONISTS", ro_dark::STATUS_FULL, ANTAGONIST_LIMIT),
                "Shown in red on the field",
                view.antagonists,
                can_manage.then_some(("Remove", 64.0, GuildAction::RemoveAntagonist as fn(u32) -> GuildAction)),
            )
        ]
    }
}

/// How a relation card ends its relation: the button's label, width and action.
type RelationEnd = Option<(&'static str, f32, fn(u32) -> GuildAction)>;

fn relation_column(
    (head, tone, limit): (&'static str, Color, usize),
    note: &'static str,
    cards: Vec<RelationCardView>,
    end: RelationEnd,
) -> impl Scene {
    let count = format!("{} / {limit}", cards.len());
    let cards: Vec<_> = cards
        .into_iter()
        .map(|card| relation_card(card, tone, end))
        .collect();
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
        }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Baseline, column_gap: px(8) }
            @ignore_picking()
            Children [
                @overline(head.into(), tone)
                --
                @chrome_text(count, 12.0, ro_dark::STRONG)
                --
                @spacer()
                --
                @chrome_text(note.into(), 11.0, ro_dark::MUTED)
            ]
            --
            {cards}
        ]
    }
}

fn relation_card(view: RelationCardView, tone: Color, end: RelationEnd) -> impl Scene {
    let (castle, castle_tone) = match view.castle {
        Some(castle) => (format!("Holds {castle}"), ro_dark::BRONZE_LIGHT),
        None => ("No castle".to_string(), ro_dark::MUTED),
    };
    let guild_id = view.guild_id;
    let end = end.map(|(label, width, action)| {
        let action = action(guild_id);
        EntityScene(bsn! {
            @row_button(ButtonArt::NORMAL, label, width)
            @usable(true, DANGER_TINT)
            action
            on(on_guild_button)
        })
    });
    let stroke = tone.with_alpha(0.4);
    bsn! {
        @card()
        Node { flex_shrink: 0.0, align_items: AlignItems::Center, column_gap: px(12) }
        BorderColor::all(stroke)
        Children [
            @emblem(guild_id, view.emblem_id, 44.0)
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
            }
            @ignore_picking()
            Children [
                @display_text(view.name, 15.0, ro_dark::HERO)
                --
                @chrome_text(view.meta, 11.0, ro_dark::MUTED)
                --
                @chrome_text(castle, 11.0, castle_tone)
            ]
            --
            {end}
        ]
    }
}

/// Leave Guild or its prompt, then the player's own tax and the storage note.
pub(super) fn footer(view: FooterView) -> impl Scene {
    let leave: Vec<Box<dyn Scene>> = if view.ask {
        vec![
            Box::new(chrome_text(view.prompt, 12.0, ro_dark::DANGER_GLOW)),
            Box::new(bsn! {
                @row_button(ButtonArt::NORMAL, "Stay", 48.0)
                GuildAction::Stay
                on(on_guild_button)
            }),
            Box::new(bsn! {
                @row_button(ButtonArt::NORMAL, "Leave Guild", 92.0)
                @usable(true, DANGER_TINT)
                GuildAction::Leave
                on(on_guild_button)
            }),
        ]
    } else {
        vec![Box::new(bsn! {
            @wide_button(ButtonArt::NORMAL, "Leave Guild", 112.0)
            @usable(true, DANGER_TINT)
            GuildAction::AskLeave
            on(on_guild_button)
        })]
    };
    let own = vec![vec![
        (ro_dark::MUTED, "Your EXP tax ".to_string()),
        (ro_dark::BRONZE_LIGHT, format!("{}%", view.tax)),
        (ro_dark::MUTED, " \u{b7} Contributed ".to_string()),
        (ro_dark::BRONZE_LIGHT, view.contributed),
    ]];
    bsn! {
        Node { flex_grow: 1.0, align_items: AlignItems::Center, column_gap: px(12) }
        @ignore_picking()
        Children [
            {leave}
            --
            @spacer()
            --
            @colored_lines(own)
            --
            @wide_button(ButtonArt::NORMAL, "Guild Storage", 124.0)
            GuildAction::Storage
            on(on_guild_button)
        ]
    }
}

pub(super) fn toast(view: ToastView) -> impl Scene {
    let (ink, stroke) = if view.error {
        (ro_dark::STATUS_FULL, ro_dark::BRONZE)
    } else {
        (ro_dark::BODY, ro_dark::BUTTON_STROKE)
    };
    bsn! {
        Node {
            padding: {UiRect::axes(px(12), px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::TOOLTIP_BODY)
        BorderColor::all(stroke)
        @ignore_picking()
        Children [ @chrome_text(view.text, 12.0, ink) ]
    }
}
