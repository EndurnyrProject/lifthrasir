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
use game_engine::domain::entities::EntityRegistry;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{PlayerAction, ui_unfocused};
use game_engine::domain::party::PartyState;
use game_engine::infrastructure::job::JobSpriteRegistry;
use leafwing_input_manager::prelude::ActionState;
use net_contract::dto::PartyMemberInfo;

use crate::theme::feathers_theme::install_norse_theme;

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

/// The swappable body region that `refresh_roster` clears and refills when visible
/// roster inputs change.
#[derive(Component, Default, Clone)]
pub struct PartyWindowBody;

/// The footer holding the Leave button; hidden while partyless so it is never a dead
/// control.
#[derive(Component, Default, Clone)]
pub struct PartyFooter;

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
        app.add_observer(crate::widgets::player_context_menu::open_player_menu);
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

/// Keep the roster closed when joining a party and hide it when leaving one.
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
    if now {
        return;
    }
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    *visibility = Visibility::Hidden;
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
/// Party snapshots own the displayed resources; `EntityRegistry` contributes only the
/// independent "on screen" chip. The rebuild is gated on change detection because the
/// partyless empty state hosts the "Create a party" button — a per-frame respawn would
/// destroy that button between a click's press and release, so it would never fire. A
/// hidden window resets `was_visible`, making the next open rebuild even when no resource
/// changed while hidden. The footer's visibility tracks membership so "Leave Party"
/// never shows while partyless.
#[allow(clippy::too_many_arguments)]
pub fn refresh_roster(
    mut commands: Commands,
    party: Res<PartyState>,
    registry: Res<EntityRegistry>,
    jobs: Option<Res<JobSpriteRegistry>>,
    root: Query<&Visibility, With<PartyWindowRoot>>,
    container: Query<(Entity, Option<&Children>), With<PartyWindowBody>>,
    mut footer: FooterQuery,
    mut was_visible: Local<bool>,
) {
    let Ok(visibility) = root.single() else {
        return;
    };
    if *visibility == Visibility::Hidden {
        *was_visible = false;
        return;
    }
    let reopened = !*was_visible;
    *was_visible = true;
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
    let jobs_changed = jobs.as_ref().is_some_and(|jobs| jobs.is_changed());
    if !empty && !reopened && !party.is_changed() && !registry.is_changed() && !jobs_changed {
        return;
    }

    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let header = party.in_party().then(|| roster_header(&party));
    let rows = roster_rows(&party, &registry, jobs.as_deref());

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

/// Project each server member snapshot into a row view-model. Job lookup is deliberately
/// quiet (`try_display_name`) because unknown jobs and a not-yet-loaded registry both use
/// the stable UI fallback. Empty when partyless (no members).
fn roster_rows(
    party: &PartyState,
    registry: &EntityRegistry,
    jobs: Option<&JobSpriteRegistry>,
) -> Vec<scene::RosterRow> {
    party
        .members
        .iter()
        .map(|member| scene::RosterRow {
            name: member.name.clone(),
            level: member.base_level,
            map: member.map.clone(),
            job_name: jobs
                .and_then(|jobs| jobs.try_display_name(member.job_id))
                .unwrap_or("Unknown Job")
                .to_string(),
            online: member.online,
            leader: member.char_id == party.leader_char_id,
            on_screen: registry.get_entity(member.char_id).is_some(),
            resources: member.online.then(|| scene::MemberResources {
                hp: scene::ResourceValue {
                    current: member.hp,
                    max: member.max_hp,
                },
                sp: scene::ResourceValue {
                    current: member.sp,
                    max: member.max_sp,
                },
                ap: (member.max_ap > 0).then(|| scene::ResourceValue {
                    current: u64::from(member.ap),
                    max: u64::from(member.max_ap),
                }),
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::infrastructure::job::JobSpriteRegistry;

    fn member(char_id: u32, online: bool) -> PartyMemberInfo {
        PartyMemberInfo {
            char_id,
            name: "Test".into(),
            base_level: 50,
            online,
            map: "prontera".into(),
            job_id: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            ap: 0,
            max_ap: 0,
        }
    }

    #[test]
    fn roster_rows_project_party_snapshot_job_and_world_presence() {
        let mut known = member(1, true);
        known.job_id = 4008;
        known.hp = u64::from(u32::MAX) + 25;
        known.max_hp = u64::from(u32::MAX) + 100;
        known.sp = 34;
        known.max_sp = 80;
        known.ap = 12;
        known.max_ap = 20;

        let mut unknown = member(2, false);
        unknown.job_id = 999_999;

        let party = PartyState {
            leader_char_id: known.char_id,
            members: vec![known, unknown],
            ..default()
        };
        let mut entities = EntityRegistry::default();
        entities.register_entity(1, Entity::from_bits(7));
        let mut job_data = lifthrasir_data::JobData::default();
        job_data
            .display_names
            .insert(4008, "Rune Knight".to_string());
        let jobs = JobSpriteRegistry::from_job_data(job_data);

        let rows = roster_rows(&party, &entities, Some(&jobs));

        assert_eq!(rows[0].job_name, "Rune Knight");
        assert!(rows[0].leader);
        assert!(rows[0].on_screen);
        assert_eq!(
            rows[0].resources,
            Some(scene::MemberResources {
                hp: scene::ResourceValue {
                    current: u64::from(u32::MAX) + 25,
                    max: u64::from(u32::MAX) + 100,
                },
                sp: scene::ResourceValue {
                    current: 34,
                    max: 80,
                },
                ap: Some(scene::ResourceValue {
                    current: 12,
                    max: 20,
                }),
            })
        );
        assert_eq!(rows[1].job_name, "Unknown Job");
        assert!(!rows[1].on_screen);
        assert_eq!(rows[1].resources, None);

        let rows_without_registry = roster_rows(&party, &entities, None);
        assert_eq!(rows_without_registry[0].job_name, "Unknown Job");
    }

    #[test]
    fn online_snapshot_omits_ap_when_max_ap_is_zero() {
        let mut online = member(1, true);
        online.ap = 8;
        online.max_ap = 0;
        let party = PartyState {
            party_id: 7,
            members: vec![online],
            ..default()
        };

        let rows = roster_rows(&party, &EntityRegistry::default(), None);

        assert!(rows[0].resources.as_ref().unwrap().ap.is_none());
    }

    fn refresh_app(member: PartyMemberInfo) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.insert_resource(PartyState {
            party_id: 7,
            name: "Wolfpack".to_string(),
            leader_char_id: member.char_id,
            members: vec![member],
            ..default()
        });
        app.init_resource::<EntityRegistry>();
        app.world_mut()
            .spawn((PartyWindowRoot, Visibility::Visible));
        app.world_mut().spawn(PartyWindowBody);
        app.world_mut().spawn((PartyFooter, Visibility::Hidden));
        app.add_systems(Update, refresh_roster);
        app
    }

    fn text_values(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect()
    }

    fn set_window_visibility(app: &mut App, visibility: Visibility) {
        *app.world_mut()
            .query_filtered::<&mut Visibility, With<PartyWindowRoot>>()
            .single_mut(app.world_mut())
            .unwrap() = visibility;
    }

    #[test]
    fn reopening_rebuilds_snapshot_changed_while_hidden() {
        let mut online = member(1, true);
        online.hp = 10;
        online.max_hp = 20;
        let mut app = refresh_app(online);

        app.update();
        assert!(text_values(&mut app).contains(&"10 / 20".to_string()));

        set_window_visibility(&mut app, Visibility::Hidden);
        app.world_mut().resource_mut::<PartyState>().members[0].hp = 15;
        app.update();
        assert!(text_values(&mut app).contains(&"10 / 20".to_string()));

        set_window_visibility(&mut app, Visibility::Visible);
        app.update();
        let texts = text_values(&mut app);
        assert!(texts.contains(&"15 / 20".to_string()));
        assert!(!texts.contains(&"10 / 20".to_string()));
    }

    #[test]
    fn job_and_entity_registry_changes_rebuild_visible_rows() {
        let mut online = member(1, true);
        online.job_id = 4008;
        let mut app = refresh_app(online);

        app.update();
        let texts = text_values(&mut app);
        assert!(texts.contains(&"Unknown Job".to_string()));
        assert!(!texts.contains(&"on screen".to_string()));

        let mut job_data = lifthrasir_data::JobData::default();
        job_data
            .display_names
            .insert(4008, "Rune Knight".to_string());
        app.insert_resource(JobSpriteRegistry::from_job_data(job_data));
        app.update();
        let texts = text_values(&mut app);
        assert!(texts.contains(&"Rune Knight".to_string()));
        assert!(!texts.contains(&"Unknown Job".to_string()));

        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(1, Entity::from_bits(7));
        app.update();
        assert!(text_values(&mut app).contains(&"on screen".to_string()));
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
    fn stays_hidden_on_join_edge_and_closes_on_leave_edge() {
        let mut app = visibility_app();

        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Hidden);

        set_party(&mut app, 7);
        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Hidden);

        set_window_visibility(&mut app, Visibility::Visible);
        set_party(&mut app, 0);
        app.update();
        assert_eq!(root_visibility(&mut app), Visibility::Hidden);
    }

    #[test]
    fn manual_close_is_not_reopened_by_same_party_update() {
        let mut app = visibility_app();

        set_party(&mut app, 7);
        app.update();
        set_window_visibility(&mut app, Visibility::Visible);
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
