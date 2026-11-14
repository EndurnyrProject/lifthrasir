//! What a unit's name plate shows, decided without touching the world.

use bevy::prelude::*;
use lifthrasir_client::domain::guild::emblems::EmblemKey;
use lifthrasir_ui::theme::ro_dark;

/// Name of a monster that attacks on sight.
const HOSTILE: Color = Color::srgb_u8(0xff, 0x9a, 0x8d);
/// Guild line of a unit outside the hero's guild.
const OTHER_GUILD: Color = Color::srgb_u8(0xe8, 0xb8, 0xa8);

/// Whose plate it is, which decides when it shows and how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlateKind {
    /// The local hero.
    Own,
    /// A member of the hero's party.
    Party,
    /// Any other player, or a player's companion.
    Player,
    Npc,
    Mob {
        aggressive: bool,
    },
    Boss,
}

impl PlateKind {
    /// Whether the plate stays up with no pointer on the unit.
    fn always_shown(self) -> bool {
        matches!(self, Self::Own | Self::Party | Self::Npc | Self::Boss)
    }

    pub fn name_color(self) -> Color {
        match self {
            Self::Own | Self::Mob { aggressive: false } => ro_dark::STRONG,
            Self::Party => ro_dark::TONE_OK,
            Self::Player => ro_dark::BODY,
            Self::Npc => ro_dark::SELECT_RING,
            Self::Mob { aggressive: true } => HOSTILE,
            Self::Boss => ro_dark::RARE_RING,
        }
    }
}

/// What draws the eye to a unit this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Focus {
    pub hovered: bool,
    /// The locked attack target.
    pub targeted: bool,
    /// The show-every-name key is held.
    pub show_all: bool,
}

/// The guild line under a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildLine {
    pub name: String,
    pub emblem: Option<EmblemKey>,
    /// The hero's own guild.
    pub allied: bool,
}

impl GuildLine {
    pub fn color(&self) -> Color {
        if self.allied {
            ro_dark::SELECT_RING
        } else {
            OTHER_GUILD
        }
    }
}

/// A unit as its plate reads it.
pub struct Unit<'a> {
    pub kind: PlateKind,
    pub name: &'a str,
    pub party: Option<&'a str>,
    pub guild: Option<GuildLine>,
}

/// Everything drawn into one plate. A plate is rebuilt when this changes; the
/// bars under it are filled in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlateView {
    pub kind: PlateKind,
    pub name: String,
    pub party: Option<String>,
    pub guild: Option<GuildLine>,
    /// The dark plate behind the lines, for the hovered or targeted unit.
    pub backed: bool,
}

impl PlateView {
    /// The plate of `unit` under `focus`, or `None` when it shows none. The hero's
    /// own plate leaves the party out and names the guild only while hovered.
    pub fn of(unit: Unit, focus: Focus) -> Option<Self> {
        let name = display_name(unit.name);
        let pointed = focus.hovered || focus.targeted;
        let shown = unit.kind.always_shown() || pointed || focus.show_all;
        if name.is_empty() || !shown {
            return None;
        }
        let own = unit.kind == PlateKind::Own;
        Some(Self {
            kind: unit.kind,
            name: name.to_string(),
            party: unit.party.filter(|_| !own).map(str::to_string),
            guild: unit.guild.filter(|_| !own || focus.hovered),
            backed: pointed,
        })
    }
}

/// The part of a unit's name the player sees: a script NPC's name hides a unique
/// suffix after `#`.
fn display_name(name: &str) -> &str {
    name.split('#').next().unwrap_or_default().trim()
}
