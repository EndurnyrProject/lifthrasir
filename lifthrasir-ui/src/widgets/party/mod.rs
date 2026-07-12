//! Party UI: the read-only roster window and (in later tasks) the create dialog,
//! invite modal, feedback lines, slash commands, and right-click invite menu. This
//! task wires only the roster window; `PartyPlugin` is the shared registration point
//! that the later tasks extend.
//!
//! The window is authored as a BSN scene ([`scene`]): persistent chrome (root,
//! titlebar, empty body region, footer with a `@FeathersButton` "Leave Party") plus a
//! swappable body that [`refresh_roster`] respawns each visible frame from a projected
//! view-model. Membership drives visibility: [`party_visibility`] opens the window only
//! on the `!in_party -> in_party` edge and closes it on the reverse, so a manual close
//! on an intermediate same-party `PartyInfo` is not fought.

use bevy::prelude::*;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::entities::EntityRegistry;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::party::PartyState;
use leafwing_input_manager::prelude::ActionState;
use net_contract::dto::PartyMemberInfo;

use crate::theme::feathers_theme::install_norse_theme;

pub mod context_menu;
pub mod create_dialog;
pub mod feedback;
pub mod invite_dialog;
pub mod scene;
pub mod slash;

pub use invite_dialog::PendingPartyInvite;
pub use slash::{PartySlash, PartySlashSubmitted};

pub use scene::build as spawn_party_window;

/// Aesir's hard cap on party size; shown as the `members/12` denominator.
pub const PARTY_MAX: usize = 12;

/// Marks the roster-window root so the toggle/visibility/refresh systems can flip it.
#[derive(Component, Default, Clone)]
pub struct PartyWindowRoot;

/// The drag-handle titlebar; the drag observer only moves the window when the drag's
/// target is the titlebar itself, so dragging from the close button is inert.
#[derive(Component, Default, Clone)]
pub struct PartyTitlebar;

/// The swappable body region that `refresh_roster` clears and refills each visible
/// frame.
#[derive(Component, Default, Clone)]
pub struct PartyWindowBody;

/// The footer holding the Leave button; hidden while partyless so it is never a dead
/// control.
#[derive(Component, Default, Clone)]
pub struct PartyFooter;

/// Whether a party member is currently resolvable to a live world entity, and if so
/// its HP. Off-screen members carry no fabricated HP — the roster shows "Elsewhere".
#[derive(Debug, PartialEq, Eq)]
pub enum MemberPresence {
    OnScreen { hp: u32, max_hp: u32 },
    Elsewhere,
}

/// A member is "on screen" only when it resolves to an entity *and* that entity carries
/// a `CharacterStatus`; either miss yields `Elsewhere` (never a made-up HP value).
pub fn member_presence(entity: Option<Entity>, status: Option<&CharacterStatus>) -> MemberPresence {
    match (entity, status) {
        (Some(_), Some(status)) => MemberPresence::OnScreen {
            hp: status.hp,
            max_hp: status.max_hp,
        },
        _ => MemberPresence::Elsewhere,
    }
}

/// Count of members flagged online by the server (the header's "active" figure).
pub fn active_count(members: &[PartyMemberInfo]) -> usize {
    members.iter().filter(|member| member.online).count()
}

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<PendingPartyInvite>();
        app.add_message::<PartySlashSubmitted>();
        app.add_observer(context_menu::open_invite_menu);
        app.add_systems(
            Update,
            toggle_party_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                party_visibility,
                refresh_roster,
                create_dialog::focus_new_name_field,
                invite_dialog::show_incoming_invite,
                invite_dialog::claim_invite_choice,
                invite_dialog::expire_pending_invite,
                feedback::ingest_party_feedback,
                slash::dispatch_party_slash,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Open the roster on the `!in_party -> in_party` edge and close it on the reverse,
/// tracking prior membership in a `Local<bool>`. Intermediate same-party `PartyInfo`
/// updates leave `in_party` unchanged, so a manually closed window is not re-opened.
pub fn party_visibility(
    party: Res<PartyState>,
    mut prev_in_party: Local<bool>,
    mut root: Query<&mut Visibility, With<PartyWindowRoot>>,
) {
    let now = party.in_party();
    if now == *prev_in_party {
        return;
    }
    *prev_in_party = now;
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    *visibility = if now {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

/// `PlayerAction::Party` toggles the window. Unconditional (works while partyless too)
/// so the "Create a party" empty state is reachable by the hotkey.
pub fn toggle_party_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<PartyWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Party) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

type FooterQuery<'w, 's> =
    Query<'w, 's, &'static mut Visibility, (With<PartyFooter>, Without<PartyWindowRoot>)>;

/// Rebuild the swappable body when the window is visible and its content actually
/// changed: despawn `PartyWindowBody`'s children and respawn the projected roster scene.
/// The HP join reads `EntityRegistry` and `CharacterStatus` live (no cached or fabricated
/// HP); an off-screen member becomes "Elsewhere". The rebuild is gated on change
/// detection because the partyless empty state hosts the "Create a party" button — a
/// per-frame respawn would destroy that button between a click's press and release, so it
/// would never fire. The footer's visibility tracks membership so "Leave Party" never
/// shows while partyless.
pub fn refresh_roster(
    mut commands: Commands,
    party: Res<PartyState>,
    registry: Res<EntityRegistry>,
    statuses: Query<&CharacterStatus>,
    changed_status: Query<(), Changed<CharacterStatus>>,
    root: Query<&Visibility, With<PartyWindowRoot>>,
    container: Query<(Entity, Option<&Children>), With<PartyWindowBody>>,
    mut footer: FooterQuery,
) {
    let Ok(visibility) = root.single() else {
        return;
    };
    if *visibility == Visibility::Hidden {
        return;
    }
    let Ok((body, children)) = container.single() else {
        return;
    };

    if let Ok(mut footer_visibility) = footer.single_mut() {
        *footer_visibility = if party.in_party() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    let empty = children.is_none_or(|children| children.is_empty());
    let hp_changed = party.in_party() && !changed_status.is_empty();
    if !empty && !party.is_changed() && !hp_changed {
        return;
    }

    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let header = party.in_party().then(|| roster_header(&party));
    let rows = roster_rows(&party, &registry, &statuses);

    commands
        .spawn_scene(scene::body(header, rows))
        .insert(ChildOf(body));
}

/// Project the party's identity + counts into the header view-model.
fn roster_header(party: &PartyState) -> scene::RosterHeader {
    let leader_name = party
        .members
        .iter()
        .find(|member| member.char_id == party.leader_char_id)
        .map(|member| member.name.clone())
        .unwrap_or_default();
    scene::RosterHeader {
        name: party.name.clone(),
        leader_name,
        members: party.members.len(),
        active: active_count(&party.members),
    }
}

/// Project each member into a row view-model, resolving the live HP join here so the
/// scene stays free of ECS queries. Empty when partyless (no members).
fn roster_rows(
    party: &PartyState,
    registry: &EntityRegistry,
    statuses: &Query<&CharacterStatus>,
) -> Vec<scene::RosterRow> {
    party
        .members
        .iter()
        .map(|member| {
            let entity = registry.get_entity(member.char_id);
            let status = entity.and_then(|entity| statuses.get(entity).ok());
            scene::RosterRow {
                name: member.name.clone(),
                level: member.base_level,
                map: member.map.clone(),
                online: member.online,
                leader: member.char_id == party.leader_char_id,
                presence: member_presence(entity, status),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(char_id: u32, online: bool) -> PartyMemberInfo {
        PartyMemberInfo {
            char_id,
            name: "Test".into(),
            base_level: 50,
            online,
            map: "prontera".into(),
        }
    }

    #[test]
    fn present_when_entity_and_status_resolve() {
        let status = CharacterStatus {
            hp: 120,
            max_hp: 200,
            ..default()
        };
        let presence = member_presence(Some(Entity::from_bits(1)), Some(&status));
        assert_eq!(
            presence,
            MemberPresence::OnScreen {
                hp: 120,
                max_hp: 200
            }
        );
    }

    #[test]
    fn elsewhere_when_entity_resolves_without_status() {
        assert_eq!(
            member_presence(Some(Entity::from_bits(1)), None),
            MemberPresence::Elsewhere
        );
    }

    #[test]
    fn elsewhere_when_entity_unresolved() {
        assert_eq!(member_presence(None, None), MemberPresence::Elsewhere);
    }

    #[test]
    fn active_count_only_counts_online() {
        let members = [member(1, true), member(2, false), member(3, true)];
        assert_eq!(active_count(&members), 2);
    }

    fn visibility_app() -> App {
        let mut app = App::new();
        app.init_resource::<PartyState>();
        app.world_mut().spawn((PartyWindowRoot, Visibility::Hidden));
        app.add_systems(Update, party_visibility);
        app
    }

    fn root_visibility(app: &mut App) -> Visibility {
        *app.world_mut()
            .query_filtered::<&Visibility, With<PartyWindowRoot>>()
            .single(app.world())
            .unwrap()
    }

    fn set_party(app: &mut App, party_id: u32) {
        app.world_mut().resource_mut::<PartyState>().party_id = party_id;
    }

    #[test]
    fn opens_on_join_edge_and_closes_on_leave_edge() {
        let mut app = visibility_app();

        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Hidden);

        set_party(&mut app, 7);
        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Visible);

        set_party(&mut app, 0);
        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Hidden);
    }

    #[test]
    fn manual_close_is_not_reopened_by_same_party_update() {
        let mut app = visibility_app();

        set_party(&mut app, 7);
        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Visible);

        for mut visibility in app
            .world_mut()
            .query_filtered::<&mut Visibility, With<PartyWindowRoot>>()
            .iter_mut(app.world_mut())
        {
            *visibility = Visibility::Hidden;
        }

        app.world_mut().resource_mut::<PartyState>().set_changed();
        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Hidden);
    }
}
