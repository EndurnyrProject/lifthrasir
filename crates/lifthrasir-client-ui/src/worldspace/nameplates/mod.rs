//! Name plates at the feet of units. NPCs, party members, bosses and the local hero
//! always carry one; every other unit shows its plate while hovered, while it is the
//! locked attack target, or while Alt is held. The hovered or targeted unit's plate
//! gets a dark backing.
//!
//! A plate is rebuilt whenever its [`PlateView`] changes; the health and spirit
//! bars under it are filled in place.

use std::collections::HashMap;

use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lifthrasir_client::domain::combat::components::DeadEntity;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::components::{EntityName, GuildIdentity};
use lifthrasir_client::domain::entities::hover::HoveredEntity;
use lifthrasir_client::domain::entities::markers::{Aggressive, Boss, LocalPlayer, Mob, Npc};
use lifthrasir_client::domain::guild::emblems::{EmblemKey, GuildEmblemImages};
use lifthrasir_client::domain::guild::{GuildState, GuildSystems};
use lifthrasir_client::domain::party::PartyState;
use net_contract::dto::PartyMemberInfo;

use crate::worldspace::{LockedUnit, WorldAnchor, WorldOverlaySystems, fill_width};
use view::{Focus, GuildLine, PlateKind, PlateView, Unit};

mod scene;
#[cfg(test)]
mod tests;
mod view;

/// Wider than any plate, so its content centres on the unit.
const PLATE_WIDTH: f32 = 260.0;
/// Pixels below the unit's projected origin (the feet), where RO shows names.
const FOOT_GAP: f32 = 6.0;
/// The base overlay layer; see [`super::WORLD_OVERLAY_Z`].
const PLATE_Z: i32 = super::WORLD_OVERLAY_Z + 100;

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                sync_nameplates,
                fill_vitals,
                request_visible_emblems,
                sync_plate_emblems,
            )
                .chain()
                .in_set(WorldOverlaySystems::Sync)
                .after(GuildSystems::UiSync)
                .before(GuildSystems::EmblemSend),
        );
    }
}

/// A unit's plate; its [`WorldAnchor`] names the unit.
#[derive(Component)]
struct Nameplate {
    view: PlateView,
}

/// A bar fill that follows one of its unit's vitals.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Vital {
    #[default]
    Hp,
    Sp,
}

/// The guild emblem image of a plate's guild line.
#[derive(Component, Default, Clone)]
struct PlateEmblem;

#[derive(QueryData)]
struct UnitRow {
    entity: Entity,
    name: &'static EntityName,
    guild: Option<&'static GuildIdentity>,
    visibility: Option<&'static Visibility>,
    own: Has<LocalPlayer>,
    npc: Has<Npc>,
    mob: Has<Mob>,
    boss: Has<Boss>,
    aggressive: Has<Aggressive>,
    hovered: Has<HoveredEntity>,
    dead: Has<DeadEntity>,
}

/// The hero's surroundings a plate depends on: who is in the party and guild, what
/// is targeted and whether every name is asked for.
#[derive(SystemParam)]
struct Roster<'w> {
    registry: Res<'w, EntityRegistry>,
    party: Res<'w, PartyState>,
    guild: Res<'w, GuildState>,
    locked: LockedUnit<'w>,
    keys: Res<'w, ButtonInput<KeyCode>>,
}

impl Roster<'_> {
    fn member(&self, unit: Entity) -> Option<&PartyMemberInfo> {
        let char_id = self.registry.get_account_id(unit)?;
        self.party
            .members
            .iter()
            .find(|member| self.party.in_party() && member.char_id == char_id)
    }

    fn kind(&self, unit: &UnitRowItem) -> PlateKind {
        if unit.own {
            return PlateKind::Own;
        }
        if unit.mob && unit.boss {
            return PlateKind::Boss;
        }
        if unit.mob {
            return PlateKind::Mob {
                aggressive: unit.aggressive,
            };
        }
        if unit.npc {
            return PlateKind::Npc;
        }
        if self.member(unit.entity).is_some() {
            return PlateKind::Party;
        }
        PlateKind::Player
    }

    /// The hero's guild comes from the guild state, anyone else's from their spawn.
    fn guild_line(&self, unit: &UnitRowItem) -> Option<GuildLine> {
        let own_guild = self.guild.info();
        if unit.own {
            return own_guild.map(|info| GuildLine {
                name: info.name.clone(),
                emblem: EmblemKey::new(info.guild_id, info.emblem_id),
                allied: true,
            });
        }
        let guild = unit.guild.filter(|guild| guild.guild_id != 0)?;
        Some(GuildLine {
            name: guild.guild_name.clone(),
            emblem: EmblemKey::new(guild.guild_id, guild.emblem_id),
            allied: own_guild.is_some_and(|info| info.guild_id == guild.guild_id),
        })
    }

    /// The plate `unit` shows this frame, if any. Hidden units and dead monsters
    /// show none.
    fn view(&self, unit: &UnitRowItem) -> Option<PlateView> {
        if unit.visibility == Some(&Visibility::Hidden) || (unit.mob && unit.dead) {
            return None;
        }
        let focus = Focus {
            hovered: unit.hovered,
            targeted: self.locked.entity() == Some(unit.entity),
            show_all: self.keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
        };
        // The name reply carries a unit's party, ours or another; our own roster
        // covers members that were never asked about.
        let party = unit
            .name
            .party_name
            .as_deref()
            .or_else(|| self.member(unit.entity).map(|_| self.party.name.as_str()));
        PlateView::of(
            Unit {
                kind: self.kind(unit),
                name: &unit.name.name,
                party,
                guild: self.guild_line(unit),
            },
            focus,
        )
    }
}

/// Keeps one plate per unit that shows one, rebuilding a plate whose view changed.
fn sync_nameplates(
    mut commands: Commands,
    roster: Roster,
    units: Query<UnitRow>,
    plates: Query<(Entity, &WorldAnchor, &Nameplate)>,
    mut wanted: Local<HashMap<Entity, PlateView>>,
) {
    wanted.clear();
    wanted.extend(
        units
            .iter()
            .filter_map(|unit| Some((unit.entity, roster.view(&unit)?))),
    );
    for (entity, anchor, plate) in &plates {
        if wanted.get(&anchor.target) == Some(&plate.view) {
            wanted.remove(&anchor.target);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (target, view) in wanted.drain() {
        commands.spawn_scene(scene::plate(view.clone())).insert((
            WorldAnchor {
                target,
                width: PLATE_WIDTH,
                drop: FOOT_GAP,
            },
            Nameplate { view },
        ));
    }
}

/// The hero's vitals come from their status, a party member's from the roster.
#[derive(SystemParam)]
struct Vitals<'w, 's> {
    hero: Query<'w, 's, &'static CharacterStatus, With<LocalPlayer>>,
    roster: Roster<'w>,
}

impl Vitals<'_, '_> {
    /// `unit`'s current and maximum `vital`, when the maximum is known.
    fn of(&self, unit: Entity, vital: Vital) -> Option<(u64, u64)> {
        let (hp, sp) = match self.hero.get(unit) {
            Ok(status) => (
                (status.hp.into(), status.max_hp.into()),
                (status.sp.into(), status.max_sp.into()),
            ),
            Err(_) => {
                let member = self.roster.member(unit)?;
                ((member.hp, member.max_hp), (member.sp, member.max_sp))
            }
        };
        let amount = match vital {
            Vital::Hp => hp,
            Vital::Sp => sp,
        };
        Some(amount).filter(|(_, limit)| *limit > 0)
    }
}

fn fill_vitals(
    vitals: Vitals,
    plates: Query<&WorldAnchor, With<Nameplate>>,
    parents: Query<&ChildOf>,
    mut fills: Query<(Entity, &Vital, &mut Node)>,
) {
    for (entity, vital, mut node) in &mut fills {
        let width = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| plates.get(ancestor).ok())
            .and_then(|anchor| vitals.of(anchor.target, *vital))
            .map(|(amount, limit)| fill_width(amount, limit));
        if let Some(width) = width.filter(|width| node.width != *width) {
            node.width = width;
        }
    }
}

/// Asks for the emblem of every guild named on a plate that is on screen.
fn request_visible_emblems(
    plates: Query<(&Nameplate, &Visibility)>,
    images: Option<ResMut<GuildEmblemImages>>,
) {
    let Some(mut images) = images else {
        return;
    };
    plates
        .iter()
        .filter(|(_, visibility)| **visibility != Visibility::Hidden)
        .filter_map(|(plate, _)| plate.view.guild.as_ref()?.emblem)
        .for_each(|key| images.request(key));
}

/// Shows each plate's emblem once its image is cached and folds it away otherwise.
fn sync_plate_emblems(
    images: Option<Res<GuildEmblemImages>>,
    plates: Query<&Nameplate>,
    parents: Query<&ChildOf>,
    mut emblems: Query<(Entity, &mut ImageNode, &mut Node), With<PlateEmblem>>,
) {
    let Some(images) = images else {
        return;
    };
    for (entity, mut image, mut node) in &mut emblems {
        let cached = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| plates.get(ancestor).ok())
            .and_then(|plate| plate.view.guild.as_ref()?.emblem)
            .and_then(|key| images.cached(key));
        let display = if cached.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        if let Some(handle) = cached.filter(|handle| image.image != *handle) {
            image.image = handle;
        }
    }
}
