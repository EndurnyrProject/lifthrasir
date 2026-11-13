//! View-models of the guild window and the guild invitation.
//!
//! Each view holds what one [`GuildHost`](super::GuildHost) region draws. They leave
//! out what a member's session changes all the time (HP, SP), so a rebuild on a
//! changed view is rare enough not to respawn a control mid-click.

use std::cmp::Ordering;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::domain::guild::GuildState;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{
    GuildActivityInfo, GuildActivityKind, GuildInfo, GuildInviteInfo, GuildMemberInfo,
    GuildPositionInfo, GuildRelationInfo, GuildRelationKind,
};

use super::{Ask, GuildTab, GuildUi, MemberSort, Perm};
use crate::screens::shared::thousands;
use crate::widgets::info_modal::description;

/// NOTE: aesir's caps on allies and antagonists. The protocol does not carry them;
/// read them from the guild snapshot once it does.
pub const ALLY_LIMIT: usize = 3;
pub const ANTAGONIST_LIMIT: usize = 3;

/// The guild as the local player `me` sees it at `now` (Unix seconds).
#[derive(Clone, Copy)]
pub struct Guild<'a> {
    pub state: &'a GuildState,
    pub info: &'a GuildInfo,
    pub me: u32,
    pub jobs: Option<&'a JobSpriteRegistry>,
    pub skills: Option<&'a SkillCatalog>,
    pub now: u64,
}

impl Guild<'_> {
    fn online(&self) -> usize {
        self.info.members.iter().filter(|m| m.online).count()
    }

    fn relations(&self, kind: GuildRelationKind) -> impl Iterator<Item = &GuildRelationInfo> {
        self.info.relations.iter().filter(move |r| r.kind == kind)
    }
}

/// A job's name, or a stable fallback while the registry is loading or the id is new.
fn job_name(jobs: Option<&JobSpriteRegistry>, job_id: u32) -> String {
    jobs.and_then(|jobs| jobs.try_display_name(job_id))
        .unwrap_or("Unknown Job")
        .to_string()
}

/// A position's title; RO leaves most of the twenty slots unnamed.
fn position_title(position: &GuildPositionInfo) -> String {
    let name = position.name.trim();
    if name.is_empty() {
        return format!("Position {}", position.index + 1);
    }
    name.to_string()
}

fn position_label(info: &GuildInfo, index: u32) -> String {
    info.positions
        .iter()
        .find(|position| position.index == index)
        .map_or_else(|| format!("Position {}", index + 1), position_title)
}

/// The master's slot in gold, then the tone of the most that a position may do.
fn position_tone(position: &GuildPositionInfo, master: bool) -> Color {
    match (master, position.can_expel, position.can_invite) {
        (true, ..) => ro_dark::RARE_RING,
        (_, true, _) => ro_dark::BRONZE_LIGHT,
        (_, _, true) => ro_dark::FROST,
        _ => ro_dark::LABEL,
    }
}

/// The slot the guild master holds, which nobody may edit or be moved into.
fn master_position(info: &GuildInfo) -> Option<u32> {
    info.members
        .iter()
        .find(|member| member.char_id == info.master_char_id)
        .map(|member| member.position_index)
}

fn master_name(info: &GuildInfo) -> String {
    info.members
        .iter()
        .find(|member| member.char_id == info.master_char_id)
        .map_or_else(|| "Unknown".to_string(), |member| member.name.clone())
}

/// How long ago `at` was: minutes, hours, days, then weeks.
pub fn ago(now: u64, at: u64) -> String {
    let mins = now.saturating_sub(at) / 60;
    match mins {
        0 => "just now".to_string(),
        1..60 => format!("{mins} min ago"),
        60..1440 => format!("{} h ago", mins / 60),
        1440..10080 => format!("{} d ago", mins / 1440),
        _ => format!("{} wk ago", mins / 10080),
    }
}

/// The castles a guild holds on one line, or `None` when it holds none.
fn castle_line(castles: &[String]) -> Option<String> {
    match castles {
        [] => None,
        [only] => Some(only.clone()),
        [first, rest @ ..] => Some(format!("{first} +{}", rest.len())),
    }
}

/// `"{count} / {max}"`, or the bare count while the capacity is unknown.
fn headcount(info: &GuildInfo) -> String {
    if info.max_members == 0 {
        return info.members.len().to_string();
    }
    format!("{} / {}", info.members.len(), info.max_members)
}

/// The subtitle in the title bar: the guild and the player's position in it.
pub fn title(guild: Guild) -> String {
    let position = guild
        .state
        .position_of(guild.me)
        .map_or_else(String::new, position_title);
    format!("{} \u{b7} {position}", guild.info.name)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabView {
    pub tab: GuildTab,
    pub label: &'static str,
    /// The count beside the label.
    pub note: String,
    pub on: bool,
}

pub fn tabs(guild: Guild, ui: &GuildUi) -> Vec<TabView> {
    let info = guild.info;
    let named = info
        .positions
        .iter()
        .filter(|position| !position.name.trim().is_empty())
        .count();
    [
        (GuildTab::Overview, "Overview", String::new()),
        (
            GuildTab::Members,
            "Members",
            format!("{} online", guild.online()),
        ),
        (GuildTab::Positions, "Positions", named.to_string()),
        (
            GuildTab::Skills,
            "Skills",
            format!("{} pts", info.skill_points),
        ),
        (
            GuildTab::Relations,
            "Relations",
            info.relations.len().to_string(),
        ),
    ]
    .into_iter()
    .map(|(tab, label, note)| TabView {
        tab,
        label,
        note,
        on: ui.selected_tab == tab,
    })
    .collect()
}

/// The skeleton of the tab on show, with what the player's position unlocks on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageView {
    Overview,
    Members { can_invite: bool },
    Positions { can_edit: bool },
    Skills,
    Relations { can_manage: bool },
}

pub fn page(guild: Guild, ui: &GuildUi) -> PageView {
    match ui.selected_tab {
        GuildTab::Overview => PageView::Overview,
        GuildTab::Members => PageView::Members {
            can_invite: guild.state.can_invite(guild.me),
        },
        GuildTab::Positions => PageView::Positions {
            can_edit: guild.state.is_master(guild.me),
        },
        GuildTab::Skills => PageView::Skills,
        GuildTab::Relations => PageView::Relations {
            can_manage: guild.state.can_relations(guild.me),
        },
    }
}

/// The guild's card and its figures, on the left of the Overview.
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryView {
    pub guild_id: u32,
    pub emblem_id: u32,
    pub name: String,
    pub level: u32,
    pub master: String,
    /// Progress to the next guild level in percent; full at the level cap.
    pub exp: f32,
    pub exp_label: String,
    /// The local player may replace the emblem.
    pub can_upload: bool,
    /// Label, value and the value's colour.
    pub stats: Vec<(&'static str, String, Color)>,
}

pub fn summary(guild: Guild) -> SummaryView {
    let info = guild.info;
    let (exp, exp_label) = if info.next_exp == 0 {
        (100.0, "MAX".to_string())
    } else {
        let exp = (info.exp as f64 * 100.0 / info.next_exp as f64).clamp(0.0, 100.0) as f32;
        (exp, format!("{}%", exp.floor()))
    };
    let levels: u64 = info.members.iter().map(|m| u64::from(m.base_level)).sum();
    let average = levels / info.members.len().max(1) as u64;
    let (castle, castle_tone) = match castle_line(&info.castles) {
        Some(castle) => (castle, ro_dark::BRONZE_LIGHT),
        None => ("None".to_string(), ro_dark::MUTED),
    };
    SummaryView {
        guild_id: info.guild_id,
        emblem_id: info.emblem_id,
        name: info.name.clone(),
        level: info.level,
        master: master_name(info),
        exp,
        exp_label,
        can_upload: guild.state.is_master(guild.me),
        stats: vec![
            ("Members", headcount(info), ro_dark::STRONG),
            (
                "Online now",
                guild.online().to_string(),
                ro_dark::STATUS_ONLINE,
            ),
            ("Average level", average.to_string(), ro_dark::STRONG),
            ("Castle", castle, castle_tone),
            (
                "Allies",
                guild.relations(GuildRelationKind::Ally).count().to_string(),
                ro_dark::FROST,
            ),
            (
                "Antagonists",
                guild
                    .relations(GuildRelationKind::Antagonist)
                    .count()
                    .to_string(),
                ro_dark::STATUS_FULL,
            ),
        ],
    }
}

/// The guild notice, or its editor.
#[derive(Clone, Debug, PartialEq)]
pub struct NoticeView {
    pub subject: String,
    pub body: String,
    /// Who posted it and when; empty when unknown and while editing, so the passing
    /// of time never rebuilds the fields being typed in.
    pub by: String,
    pub can_edit: bool,
    pub editing: bool,
}

fn posted_by(info: &GuildInfo, now: u64) -> String {
    if info.notice_author.is_empty() {
        return String::new();
    }
    if info.notice_posted_at == 0 {
        return format!("Posted by {}", info.notice_author);
    }
    format!(
        "Posted by {} \u{b7} {}",
        info.notice_author,
        ago(now, info.notice_posted_at)
    )
}

pub fn notice(guild: Guild, ui: &GuildUi) -> NoticeView {
    let info = guild.info;
    let can_edit = guild.state.can_notice(guild.me);
    let editing = ui.editing_notice && can_edit;
    NoticeView {
        subject: info.notice_subject.clone(),
        body: info.notice_body.clone(),
        by: if editing {
            String::new()
        } else {
            posted_by(info, guild.now)
        },
        can_edit,
        editing,
    }
}

/// One line of the guild's recent activity.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityLine {
    pub text: String,
    pub when: String,
    pub tone: Color,
}

fn skill_name(skills: Option<&SkillCatalog>, skill_id: u32) -> String {
    skills
        .and_then(|skills| skills.get(skill_id))
        .filter(|meta| !meta.display_name.is_empty())
        .map_or_else(
            || format!("Skill {skill_id}"),
            |meta| meta.display_name.clone(),
        )
}

/// What an entry says and the colour of its mark; `None` for a kind this client
/// does not know.
fn activity_text(
    entry: &GuildActivityInfo,
    skills: Option<&SkillCatalog>,
) -> Option<(String, Color)> {
    let (subject, actor) = (&entry.subject, &entry.actor);
    let by = |verb: &str| {
        if actor.is_empty() {
            String::new()
        } else {
            format!(" \u{b7} {verb} {actor}")
        }
    };
    Some(match entry.kind {
        GuildActivityKind::Created => (format!("{actor} founded the guild"), ro_dark::RARE_RING),
        GuildActivityKind::Joined => (
            format!("{subject} joined the guild{}", by("Invited by")),
            ro_dark::STATUS_ONLINE,
        ),
        GuildActivityKind::Left => (format!("{subject} left the guild"), ro_dark::MUTED),
        GuildActivityKind::Expelled => (
            format!("{subject} was expelled{}", by("By")),
            ro_dark::STATUS_FULL,
        ),
        GuildActivityKind::PositionChanged => (
            format!("{subject} was made {}", entry.detail),
            ro_dark::FROST,
        ),
        GuildActivityKind::SkillLearned => (
            format!(
                "{actor} learned {} Lv {}",
                skill_name(skills, entry.skill_id),
                entry.level
            ),
            ro_dark::FROST,
        ),
        GuildActivityKind::LevelUp => (
            format!("The guild reached Lv {}", entry.level),
            ro_dark::RARE_RING,
        ),
        GuildActivityKind::AllianceFormed => {
            (format!("Alliance formed with {subject}"), ro_dark::FROST)
        }
        GuildActivityKind::AllianceBroken => {
            (format!("Alliance with {subject} ended"), ro_dark::MUTED)
        }
        GuildActivityKind::AntagonistDeclared => (
            format!("{subject} declared as antagonists"),
            ro_dark::STATUS_FULL,
        ),
        GuildActivityKind::AntagonistRemoved => (
            format!("{subject} are no longer antagonists"),
            ro_dark::MUTED,
        ),
        GuildActivityKind::CastleTaken => (format!("The guild took {subject}"), ro_dark::RARE_RING),
        GuildActivityKind::CastleLost => {
            (format!("The guild lost {subject}"), ro_dark::STATUS_FULL)
        }
        GuildActivityKind::Unknown(_) => return None,
    })
}

pub fn activity(guild: Guild) -> Vec<ActivityLine> {
    guild
        .info
        .activity
        .iter()
        .filter_map(|entry| {
            let (text, tone) = activity_text(entry, guild.skills)?;
            Some(ActivityLine {
                text,
                when: ago(guild.now, entry.at),
                tone,
            })
        })
        .collect()
}

/// The filter chip and the head count over the roster.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolsView {
    pub online_only: bool,
    pub count: String,
}

pub fn tools(guild: Guild, ui: &GuildUi) -> ToolsView {
    ToolsView {
        online_only: ui.online_only,
        count: format!("{} online of {}", guild.online(), guild.info.members.len()),
    }
}

/// One roster row.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberRowView {
    pub char_id: u32,
    pub name: String,
    pub me: bool,
    pub online: bool,
    pub selected: bool,
    pub job: String,
    pub level: u32,
    pub position: String,
    pub position_tone: Color,
    /// EXP paid to the guild, grouped by thousands.
    pub exp: String,
    /// The map while online, else how long ago they were.
    pub seen: String,
}

fn seen(member: &GuildMemberInfo, now: u64) -> String {
    if member.online {
        return member.map.clone();
    }
    if member.last_seen == 0 {
        return "Offline".to_string();
    }
    ago(now, member.last_seen)
}

/// The order of two members under `sort`, before the name breaks ties.
fn member_order(
    sort: MemberSort,
    (a, a_job): (&GuildMemberInfo, &str),
    (b, b_job): (&GuildMemberInfo, &str),
) -> Ordering {
    match sort {
        MemberSort::Name => Ordering::Equal,
        MemberSort::Job => a_job.cmp(b_job),
        MemberSort::Level => b.base_level.cmp(&a.base_level),
        MemberSort::Position => a.position_index.cmp(&b.position_index),
        MemberSort::Exp => b.exp.cmp(&a.exp),
        // Online first by rank, then whoever was seen last.
        MemberSort::Seen => b.online.cmp(&a.online).then_with(|| {
            if a.online {
                a.position_index.cmp(&b.position_index)
            } else {
                b.last_seen.cmp(&a.last_seen)
            }
        }),
    }
}

/// The roster filtered by `query` (name, job or map) and the online chip, in the
/// picked order.
pub fn member_rows(guild: Guild, ui: &GuildUi, query: &str) -> Vec<MemberRowView> {
    let info = guild.info;
    let query = query.trim().to_lowercase();
    let mut members: Vec<(&GuildMemberInfo, String)> = info
        .members
        .iter()
        .filter(|member| member.online || !ui.online_only)
        .map(|member| (member, job_name(guild.jobs, member.job_id)))
        .filter(|(member, job)| {
            query.is_empty()
                || format!("{} {job} {}", member.name, member.map)
                    .to_lowercase()
                    .contains(&query)
        })
        .collect();
    members.sort_by(|(a, a_job), (b, b_job)| {
        member_order(ui.sort, (a, a_job), (b, b_job))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    if ui.sort_reversed {
        members.reverse();
    }
    members
        .into_iter()
        .map(|(member, job)| {
            let position = info
                .positions
                .iter()
                .find(|position| position.index == member.position_index);
            MemberRowView {
                char_id: member.char_id,
                name: member.name.clone(),
                me: member.char_id == guild.me,
                online: member.online,
                selected: ui.selected_member == Some(member.char_id),
                job,
                level: member.base_level,
                position: position_label(info, member.position_index),
                position_tone: position.map_or(ro_dark::LABEL, |position| {
                    position_tone(position, member.char_id == info.master_char_id)
                }),
                exp: thousands(member.exp),
                seen: seen(member, guild.now),
            }
        })
        .collect()
}

/// The bar under the roster acting on the selected member.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionView {
    pub char_id: u32,
    pub name: String,
    pub title: String,
    pub meta: String,
    pub online: bool,
    pub can_whisper: bool,
    pub can_party: bool,
    pub position: String,
    pub can_assign: bool,
    /// The positions the member can be moved to, while the position menu is open:
    /// slot, title and whether they hold it.
    pub menu: Vec<(u32, String, bool)>,
    pub can_expel: bool,
    /// The expel prompt is up.
    pub ask_expel: bool,
}

pub fn selection(guild: Guild, ui: &GuildUi) -> Option<SelectionView> {
    let info = guild.info;
    let member = guild.state.member(ui.selected_member?)?;
    let me = member.char_id == guild.me;
    let master = member.char_id == info.master_char_id;
    let position = position_label(info, member.position_index);
    let place = match (member.online, member.last_seen) {
        (true, _) => format!("Online in {}", member.map),
        (false, 0) => "Offline".to_string(),
        (false, at) => format!("Last seen {}", ago(guild.now, at)),
    };
    let can_assign = guild.state.is_master(guild.me) && !master;
    let protected = master_position(info);
    let menu = if ui.position_menu && can_assign {
        info.positions
            .iter()
            .filter(|p| Some(p.index) != protected && !p.name.trim().is_empty())
            .map(|p| (p.index, position_title(p), p.index == member.position_index))
            .collect()
    } else {
        Vec::new()
    };
    Some(SelectionView {
        char_id: member.char_id,
        name: member.name.clone(),
        title: format!("{}{}", member.name, if me { " (you)" } else { "" }),
        meta: format!(
            "{} \u{b7} Base Lv {} \u{b7} {position} \u{b7} {place}",
            job_name(guild.jobs, member.job_id),
            member.base_level
        ),
        online: member.online,
        can_whisper: !me,
        can_party: !me && member.online,
        position,
        can_assign,
        menu,
        can_expel: !me && !master && guild.state.can_expel(guild.me),
        ask_expel: ui.ask == Some(Ask::Expel),
    })
}

/// One row of the positions table.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionRowView {
    pub index: u32,
    pub name: String,
    pub tone: Color,
    /// The local player holds it.
    pub mine: bool,
    pub count: usize,
    /// What it may do, in [`Perm::ALL`] order.
    pub perms: [bool; 5],
    pub tax: u32,
    /// The master's own slot, which stays as it is.
    pub protected: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PositionsView {
    pub rows: Vec<PositionRowView>,
    /// The local player is the master and may edit the unprotected rows.
    pub can_edit: bool,
}

/// The master sees every slot so the blank ones can be named; everyone else sees the
/// slots that are named or held.
pub fn positions(guild: Guild, ui: &GuildUi) -> PositionsView {
    let info = guild.info;
    let can_edit = guild.state.is_master(guild.me);
    let protected = master_position(info);
    let mine = guild.state.member(guild.me).map(|m| m.position_index);
    let rows = info
        .positions
        .iter()
        .map(|position| {
            let held = Some(position.index) == protected;
            let count = info
                .members
                .iter()
                .filter(|member| member.position_index == position.index)
                .count();
            PositionRowView {
                index: position.index,
                name: position_title(position),
                tone: position_tone(position, held),
                mine: mine == Some(position.index),
                count,
                perms: Perm::ALL.map(|perm| held || perm.of(position)),
                tax: position.tax,
                protected: held,
                selected: can_edit && ui.selected_position == Some(position.index),
            }
        })
        .filter(|row| can_edit || row.count > 0 || !position_blank(info, row.index))
        .collect();
    PositionsView { rows, can_edit }
}

fn position_blank(info: &GuildInfo, index: u32) -> bool {
    info.positions
        .iter()
        .find(|position| position.index == index)
        .is_none_or(|position| position.name.trim().is_empty())
}

/// The bar that renames the selected position and sets its tax.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionEditView {
    pub index: u32,
    pub name: String,
    pub tax: u32,
}

pub fn position_edit(guild: Guild, ui: &GuildUi) -> Option<PositionEditView> {
    if !guild.state.is_master(guild.me) {
        return None;
    }
    let index = ui.selected_position?;
    if Some(index) == master_position(guild.info) {
        return None;
    }
    let position = guild.state.position(index)?;
    Some(PositionEditView {
        index,
        name: position.name.clone(),
        tax: position.tax,
    })
}

/// One guild skill card.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillCardView {
    pub skill_id: u32,
    pub name: String,
    pub icon: Option<String>,
    pub level: String,
    /// The first sentence of the catalog's description.
    pub blurb: String,
    pub learned: bool,
    /// The tag of an active skill and its colour; passives have none.
    pub state: Option<(&'static str, Color)>,
    pub can_upgrade: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillsView {
    pub points: u32,
    pub cards: Vec<SkillCardView>,
}

/// The catalog marks passives on the "Skill Form" line of their description.
fn is_active(description: &[String]) -> bool {
    description
        .iter()
        .any(|line| line.starts_with("Skill Form:") && !line.contains("Passive"))
}

/// The first sentence of a catalog description for the skill called `name`.
fn blurb(lines: &[String], name: &str) -> String {
    let text = description::parse(lines, name)
        .into_iter()
        .map(|runs| runs.into_iter().map(|(_, text)| text).collect::<String>())
        .find_map(|line| {
            line.strip_prefix("Description:")
                .map(|rest| rest.trim().to_string())
        })
        .unwrap_or_default();
    match text.split_once(". ") {
        Some((first, _)) => format!("{first}."),
        None => text,
    }
}

pub fn skills(guild: Guild) -> SkillsView {
    let info = guild.info;
    let master = guild.state.is_master(guild.me);
    let cards = info
        .skills
        .iter()
        .map(|skill| {
            let meta = guild.skills.and_then(|skills| skills.get(skill.skill_id));
            let learned = skill.level > 0;
            let active = meta.is_some_and(|meta| is_active(&meta.description));
            let state = active.then_some(if learned {
                ("Active", ro_dark::FROST)
            } else {
                ("Not learned", ro_dark::MUTED)
            });
            SkillCardView {
                skill_id: skill.skill_id,
                name: skill_name(guild.skills, skill.skill_id),
                icon: guild
                    .skills
                    .and_then(|skills| skills.icon_path(skill.skill_id)),
                level: format!("{} / {}", skill.level, skill.max_level),
                blurb: meta
                    .map(|meta| blurb(&meta.description, &meta.display_name))
                    .unwrap_or_default(),
                learned,
                state,
                can_upgrade: master
                    && info.skill_points > 0
                    && skill.max_level > 0
                    && skill.level < skill.max_level,
            }
        })
        .collect();
    SkillsView {
        points: info.skill_points,
        cards,
    }
}

/// One allied or antagonist guild.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationCardView {
    pub guild_id: u32,
    pub emblem_id: u32,
    pub name: String,
    pub meta: String,
    /// The castles it holds, or `None`.
    pub castle: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelationsView {
    pub allies: Vec<RelationCardView>,
    pub antagonists: Vec<RelationCardView>,
    /// The local player may end a relation.
    pub can_manage: bool,
}

fn relation_card(relation: &GuildRelationInfo) -> RelationCardView {
    let members = if relation.member_count == 1 {
        "1 member".to_string()
    } else {
        format!("{} members", relation.member_count)
    };
    RelationCardView {
        guild_id: relation.guild_id,
        emblem_id: relation.emblem_id,
        name: relation.name.clone(),
        meta: format!(
            "Lv {} \u{b7} {members} \u{b7} Master {}",
            relation.level, relation.master_name
        ),
        castle: castle_line(&relation.castles),
    }
}

pub fn relations(guild: Guild) -> RelationsView {
    let cards = |kind| guild.relations(kind).map(relation_card).collect();
    RelationsView {
        allies: cards(GuildRelationKind::Ally),
        antagonists: cards(GuildRelationKind::Antagonist),
        can_manage: guild.state.can_relations(guild.me),
    }
}

/// The row under the panel: Leave Guild or its prompt, then the player's own tax.
#[derive(Clone, Debug, PartialEq)]
pub struct FooterView {
    /// The leave prompt is up.
    pub ask: bool,
    pub prompt: String,
    pub tax: u32,
    /// EXP the local player has paid to the guild.
    pub contributed: String,
}

pub fn footer(guild: Guild, ui: &GuildUi) -> FooterView {
    let name = &guild.info.name;
    FooterView {
        ask: ui.ask == Some(Ask::Leave),
        prompt: if guild.state.is_master(guild.me) {
            format!("Leaving disbands {name}.")
        } else {
            format!("Leave {name}? You\u{2019}ll need a new invite to come back.")
        },
        tax: guild.state.position_of(guild.me).map_or(0, |p| p.tax),
        contributed: thousands(guild.state.member(guild.me).map_or(0, |m| m.exp)),
    }
}

/// The message floating over the panel.
#[derive(Clone, Debug, PartialEq)]
pub struct ToastView {
    pub text: String,
    pub error: bool,
}

pub fn toast(ui: &GuildUi) -> Option<ToastView> {
    Some(ToastView {
        text: ui.feedback.clone()?,
        error: ui.feedback_is_error,
    })
}

/// A guild invitation: who invites and the guild they invite to.
#[derive(Clone, Debug, PartialEq)]
pub struct InviteView {
    pub guild_id: u32,
    pub emblem_id: u32,
    pub inviter: String,
    /// The inviter's job, when they are on the guild's roster.
    pub job: Option<String>,
    /// What else is known about the inviter.
    pub about: String,
    pub guild: String,
    pub level: u32,
    pub master: String,
    pub members: String,
    pub online: String,
    pub castle: Option<String>,
    /// The position a new member starts in.
    pub join_as: String,
    pub tax: String,
    pub allies: String,
    pub antagonists: String,
}

pub fn invite(invite: &GuildInviteInfo, jobs: Option<&JobSpriteRegistry>) -> InviteView {
    let info = &invite.guild;
    let inviter = info
        .members
        .iter()
        .find(|member| member.char_id == invite.inviter_char_id);
    let about = inviter.map_or_else(String::new, |inviter| {
        format!(
            "Base Lv {} \u{b7} {} of the guild",
            inviter.base_level,
            position_label(info, inviter.position_index)
        )
    });
    // New members start in the last slot.
    let newcomer = info.positions.iter().max_by_key(|position| position.index);
    let names = |kind| {
        let names: Vec<_> = info
            .relations
            .iter()
            .filter(|relation| relation.kind == kind)
            .map(|relation| relation.name.as_str())
            .collect();
        if names.is_empty() {
            "None".to_string()
        } else {
            names.join(", ")
        }
    };
    InviteView {
        guild_id: invite.guild_id,
        emblem_id: info.emblem_id,
        inviter: invite.inviter_name.clone(),
        job: inviter.map(|inviter| job_name(jobs, inviter.job_id)),
        about,
        guild: invite.guild_name.clone(),
        level: info.level,
        master: master_name(info),
        members: headcount(info),
        online: info.members.iter().filter(|m| m.online).count().to_string(),
        castle: castle_line(&info.castles),
        join_as: newcomer.map_or_else(String::new, position_title),
        tax: format!("{}%", newcomer.map_or(0, |position| position.tax)),
        allies: names(GuildRelationKind::Ally),
        antagonists: names(GuildRelationKind::Antagonist),
    }
}

/// What the invitation turns into once the player is in the guild.
#[derive(Clone, Debug, PartialEq)]
pub struct WelcomeView {
    pub guild: String,
    pub position: String,
    pub notice: NoticeView,
}

pub fn welcome(guild: Guild) -> WelcomeView {
    WelcomeView {
        guild: guild.info.name.clone(),
        position: guild
            .state
            .position_of(guild.me)
            .map_or_else(String::new, position_title),
        notice: NoticeView {
            subject: guild.info.notice_subject.clone(),
            body: guild.info.notice_body.clone(),
            by: posted_by(guild.info, guild.now),
            can_edit: false,
            editing: false,
        },
    }
}
