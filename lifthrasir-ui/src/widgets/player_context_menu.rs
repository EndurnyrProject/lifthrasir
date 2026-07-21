//! One right-click popup for independently eligible Party and Guild invitations.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::entities::components::NetworkEntity;
use game_engine::domain::entities::types::ObjectType;
use game_engine::domain::guild::GuildState;
use game_engine::domain::party::PartyState;
use net_contract::commands::{GuildInviteRequested, PartyInviteRequested};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::theme;
use crate::widgets::guild_window::{GuildUi, request_invite};

const MENU_Z: i32 = i32::MAX - 5;
const MENU_WIDTH: f32 = 168.0;

#[derive(Component, Default, Clone)]
pub struct PlayerContextMenuRoot;

#[derive(Component, Default, Clone)]
pub struct ContextMenuTarget(pub u32);

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum InviteAction {
    #[default]
    Party,
    Guild,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerMenuActions {
    pub party: bool,
    pub guild: bool,
}

impl PlayerMenuActions {
    fn any(self) -> bool {
        self.party || self.guild
    }
}

fn pick_root(child: Entity, child_of: &Query<&ChildOf>) -> Entity {
    child_of.get(child).map(|c| c.parent()).unwrap_or(child)
}

pub fn eligible_actions(
    button: PointerButton,
    net: Option<&NetworkEntity>,
    is_local: bool,
    party: bool,
    guild: bool,
) -> PlayerMenuActions {
    let valid_target = button == PointerButton::Secondary
        && !is_local
        && matches!(net, Some(net) if net.object_type == ObjectType::Pc);
    if !valid_target {
        return PlayerMenuActions::default();
    }
    PlayerMenuActions { party, guild }
}

#[allow(clippy::too_many_arguments)]
pub fn open_player_menu(
    mut click: On<Pointer<Click>>,
    child_of: Query<&ChildOf>,
    nets: Query<&NetworkEntity>,
    session: Res<ZoneSession>,
    party: Res<PartyState>,
    guild: Res<GuildState>,
    guild_ui: Res<GuildUi>,
    existing: Query<Entity, With<PlayerContextMenuRoot>>,
    mut commands: Commands,
) {
    let root = pick_root(click.entity, &child_of);
    let net = nets.get(root).ok();
    let is_local = net.is_some_and(|net| net.gid == session.char_id);
    let actions = eligible_actions(
        click.event.button,
        net,
        is_local,
        party.is_leader(session.char_id),
        guild_ui.pending.is_none() && guild.can_invite(session.char_id),
    );
    if !actions.any() {
        return;
    }

    let target = net.expect("eligible_actions requires a PC").gid;
    click.propagate(false);
    for menu in &existing {
        commands.entity(menu).despawn();
    }
    commands.spawn_scene(context_menu(
        click.pointer_location.position,
        target,
        actions,
    ));
}

fn context_menu(cursor: Vec2, target: u32, actions: PlayerMenuActions) -> impl Scene {
    bsn! {
        PlayerContextMenuRoot
        ContextMenuTarget({target})
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        GlobalZIndex({MENU_Z})
        Pickable
        on(dismiss_menu)
        Children [ card(cursor, actions) ]
    }
}

fn card(cursor: Vec2, actions: PlayerMenuActions) -> impl Scene {
    let mut buttons = Vec::new();
    if actions.party {
        buttons.push(invite_button("Invite to Party", InviteAction::Party));
    }
    if actions.guild {
        buttons.push(invite_button("Invite to Guild", InviteAction::Guild));
    }
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {px(cursor.x)},
            top: {px(cursor.y)},
            width: px(MENU_WIDTH),
            padding: {UiRect::all(px(6))},
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor({theme::GLASS})
        BorderColor::all(theme::STROKE)
        Pickable
        on(|mut click: On<Pointer<Click>>| click.propagate(false))
        Children [ {buttons} ]
    }
}

fn invite_button(label: &'static str, action: InviteAction) -> impl Scene {
    bsn! {
        template_value(action)
        @FeathersButton {
            @caption: bsn! {
                (
                    Text(label)
                    TextFont {
                        font: FontSourceTemplate::Handle(theme::FONT_BODY),
                        font_size: {FontSize::Px(14.0)},
                    }
                    ThemedText
                )
            },
            @variant: ButtonVariant::Primary,
        }
        Node { height: px(36), border_radius: BorderRadius::all(px(7)) }
        on(on_invite)
    }
}

#[allow(clippy::too_many_arguments)]
fn on_invite(
    activate: On<Activate>,
    action: Query<&InviteAction>,
    menu: Query<(Entity, &ContextMenuTarget), With<PlayerContextMenuRoot>>,
    generation: Res<ZoneSessionGeneration>,
    mut guild_ui: ResMut<GuildUi>,
    mut party_writer: MessageWriter<PartyInviteRequested>,
    mut guild_writer: MessageWriter<GuildInviteRequested>,
    mut commands: Commands,
) {
    let Ok(action) = action.get(activate.entity) else {
        return;
    };
    let Ok((root, target)) = menu.single() else {
        return;
    };
    match action {
        InviteAction::Party => {
            party_writer.write(PartyInviteRequested {
                target_char_id: target.0,
                target_name: String::new(),
            });
        }
        InviteAction::Guild => {
            if let Some(command) = request_invite(&mut guild_ui, *generation, target.0, "") {
                guild_writer.write(command);
            }
        }
    }
    commands.entity(root).despawn();
}

fn dismiss_menu(
    _: On<Pointer<Click>>,
    menu: Query<Entity, With<PlayerContextMenuRoot>>,
    mut commands: Commands,
) {
    if let Ok(root) = menu.single() {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn pc() -> NetworkEntity {
        NetworkEntity::new(42, 42, ObjectType::Pc)
    }

    #[test]
    fn party_and_guild_eligibility_are_independent() {
        for (party, guild) in [(true, false), (false, true), (true, true), (false, false)] {
            assert_eq!(
                eligible_actions(PointerButton::Secondary, Some(&pc()), false, party, guild),
                PlayerMenuActions { party, guild }
            );
        }
    }

    fn rendered_actions(actions: PlayerMenuActions) -> Vec<String> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut()
            .spawn_scene(card(Vec2::ZERO, actions))
            .unwrap();
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn menu_renders_party_only_guild_only_both_or_neither() {
        assert_eq!(
            rendered_actions(PlayerMenuActions {
                party: true,
                guild: false
            }),
            ["Invite to Party"]
        );
        assert_eq!(
            rendered_actions(PlayerMenuActions {
                party: false,
                guild: true
            }),
            ["Invite to Guild"]
        );
        assert_eq!(
            rendered_actions(PlayerMenuActions {
                party: true,
                guild: true
            }),
            ["Invite to Party", "Invite to Guild"]
        );
        assert!(rendered_actions(PlayerMenuActions::default()).is_empty());
    }

    #[test]
    fn local_non_pc_and_primary_clicks_are_never_claimed() {
        let mob = NetworkEntity::new(2, 2, ObjectType::Mob);
        assert_eq!(
            eligible_actions(PointerButton::Secondary, Some(&pc()), true, true, true),
            PlayerMenuActions::default()
        );
        assert_eq!(
            eligible_actions(PointerButton::Secondary, Some(&mob), false, true, true),
            PlayerMenuActions::default()
        );
        assert_eq!(
            eligible_actions(PointerButton::Primary, Some(&pc()), false, true, true),
            PlayerMenuActions::default()
        );
    }

    #[test]
    fn party_action_preserves_the_existing_command_payload() {
        let mut app = App::new();
        app.add_message::<PartyInviteRequested>()
            .add_message::<GuildInviteRequested>()
            .insert_resource(ZoneSessionGeneration(3))
            .init_resource::<GuildUi>();
        app.world_mut()
            .spawn((PlayerContextMenuRoot, ContextMenuTarget(1337)));
        let button = app
            .world_mut()
            .spawn(InviteAction::Party)
            .observe(on_invite)
            .id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<PartyInviteRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).collect();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target_char_id, 1337);
        assert_eq!(written[0].target_name, "");
    }

    #[test]
    fn guild_action_uses_the_same_invite_request_as_by_name() {
        let mut app = App::new();
        app.add_message::<PartyInviteRequested>()
            .add_message::<GuildInviteRequested>()
            .insert_resource(ZoneSessionGeneration(3))
            .init_resource::<GuildUi>();
        app.world_mut()
            .spawn((PlayerContextMenuRoot, ContextMenuTarget(1337)));
        let button = app
            .world_mut()
            .spawn(InviteAction::Guild)
            .observe(on_invite)
            .id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<GuildInviteRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).collect();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target_char_id, 1337);
        assert_eq!(written[0].target_name, "");
    }
}
