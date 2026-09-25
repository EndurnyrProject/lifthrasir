//! One right-click popup for eligible Party, Guild, and Trade actions.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::entities::components::NetworkEntity;
use game_engine::domain::entities::types::ObjectType;
use game_engine::domain::guild::GuildState;
use game_engine::domain::party::PartyState;
use game_engine::domain::trade::TradeSession;
use net_contract::commands::{GuildInviteRequested, PartyInviteRequested, RequestTrade};
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
enum MenuAction {
    #[default]
    Party,
    Guild,
    Trade,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerMenuActions {
    pub party: bool,
    pub guild: bool,
    pub trade: bool,
}

impl PlayerMenuActions {
    fn any(self) -> bool {
        self.party || self.guild || self.trade
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
    trade: bool,
) -> PlayerMenuActions {
    let valid_target = button == PointerButton::Secondary
        && !is_local
        && matches!(net, Some(net) if net.object_type == ObjectType::Pc);
    if !valid_target {
        return PlayerMenuActions::default();
    }
    PlayerMenuActions {
        party,
        guild,
        trade,
    }
}

/// Social state that decides which menu actions the local player may use.
#[derive(SystemParam)]
pub struct MenuEligibility<'w> {
    session: Res<'w, ZoneSession>,
    party: Res<'w, PartyState>,
    guild: Res<'w, GuildState>,
    guild_ui: Res<'w, GuildUi>,
    trade: Res<'w, TradeSession>,
}

pub fn open_player_menu(
    mut click: On<Pointer<Click>>,
    child_of: Query<&ChildOf>,
    nets: Query<&NetworkEntity>,
    state: MenuEligibility,
    existing: Query<Entity, With<PlayerContextMenuRoot>>,
    mut commands: Commands,
) {
    let root = pick_root(click.entity, &child_of);
    let net = nets.get(root).ok();
    let me = state.session.char_id;
    let actions = eligible_actions(
        click.event.button,
        net,
        net.is_some_and(|net| net.gid == me),
        state.party.is_leader(me),
        state.guild_ui.pending.is_none() && state.guild.can_invite(me),
        !state.trade.is_open(),
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
        buttons.push(invite_button("Invite to Party", MenuAction::Party));
    }
    if actions.guild {
        buttons.push(invite_button("Invite to Guild", MenuAction::Guild));
    }
    if actions.trade {
        buttons.push(invite_button("Trade", MenuAction::Trade));
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

fn invite_button(label: &'static str, action: MenuAction) -> impl Scene {
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

/// Everything a menu button needs to turn its action into an outbound command.
#[derive(SystemParam)]
struct InviteSender<'w> {
    generation: Res<'w, ZoneSessionGeneration>,
    trade: Res<'w, TradeSession>,
    guild_ui: ResMut<'w, GuildUi>,
    party_writer: MessageWriter<'w, PartyInviteRequested>,
    guild_writer: MessageWriter<'w, GuildInviteRequested>,
    trade_writer: MessageWriter<'w, RequestTrade>,
}

impl InviteSender<'_> {
    fn send(&mut self, action: MenuAction, target: u32) {
        match action {
            MenuAction::Party => {
                self.party_writer.write(PartyInviteRequested {
                    target_char_id: target,
                    target_name: String::new(),
                });
            }
            MenuAction::Guild => {
                let generation = *self.generation;
                if let Some(command) = request_invite(&mut self.guild_ui, generation, target, "") {
                    self.guild_writer.write(command);
                }
            }
            MenuAction::Trade if !self.trade.is_open() => {
                self.trade_writer.write(RequestTrade {
                    target_char_id: target,
                });
            }
            MenuAction::Trade => {}
        }
    }
}

fn on_invite(
    activate: On<Activate>,
    action: Query<&MenuAction>,
    menu: Query<(Entity, &ContextMenuTarget), With<PlayerContextMenuRoot>>,
    mut sender: InviteSender,
    mut commands: Commands,
) {
    let Ok(action) = action.get(activate.entity) else {
        return;
    };
    let Ok((root, target)) = menu.single() else {
        return;
    };
    sender.send(*action, target.0);
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
                eligible_actions(
                    PointerButton::Secondary,
                    Some(&pc()),
                    false,
                    party,
                    guild,
                    false
                ),
                PlayerMenuActions {
                    party,
                    guild,
                    trade: false
                }
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
                guild: false,
                trade: false,
            }),
            ["Invite to Party"]
        );
        assert_eq!(
            rendered_actions(PlayerMenuActions {
                party: false,
                guild: true,
                trade: false,
            }),
            ["Invite to Guild"]
        );
        assert_eq!(
            rendered_actions(PlayerMenuActions {
                party: true,
                guild: true,
                trade: false,
            }),
            ["Invite to Party", "Invite to Guild"]
        );
        assert!(rendered_actions(PlayerMenuActions::default()).is_empty());
    }

    #[test]
    fn local_non_pc_and_primary_clicks_are_never_claimed() {
        let mob = NetworkEntity::new(2, 2, ObjectType::Mob);
        assert_eq!(
            eligible_actions(
                PointerButton::Secondary,
                Some(&pc()),
                true,
                true,
                true,
                true
            ),
            PlayerMenuActions::default()
        );
        assert_eq!(
            eligible_actions(
                PointerButton::Secondary,
                Some(&mob),
                false,
                true,
                true,
                true
            ),
            PlayerMenuActions::default()
        );
        assert_eq!(
            eligible_actions(PointerButton::Primary, Some(&pc()), false, true, true, true),
            PlayerMenuActions::default()
        );
    }

    #[test]
    fn trade_only_available_for_other_pc_and_visible_in_menu() {
        assert_eq!(
            eligible_actions(
                PointerButton::Secondary,
                Some(&pc()),
                false,
                false,
                false,
                true
            ),
            PlayerMenuActions {
                party: false,
                guild: false,
                trade: true
            }
        );
        assert!(
            !eligible_actions(
                PointerButton::Secondary,
                Some(&pc()),
                false,
                false,
                false,
                false
            )
            .trade
        );
        assert_eq!(
            rendered_actions(PlayerMenuActions {
                party: false,
                guild: false,
                trade: true
            }),
            ["Trade"]
        );
    }

    #[test]
    fn trade_action_sends_clicked_gid_and_closes_menu() {
        let mut app = App::new();
        app.add_message::<PartyInviteRequested>()
            .add_message::<GuildInviteRequested>()
            .add_message::<RequestTrade>()
            .insert_resource(ZoneSessionGeneration(3))
            .init_resource::<TradeSession>()
            .init_resource::<GuildUi>();
        let menu = app
            .world_mut()
            .spawn((PlayerContextMenuRoot, ContextMenuTarget(1337)))
            .id();
        let button = app
            .world_mut()
            .spawn(MenuAction::Trade)
            .observe(on_invite)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();
        let sent: Vec<_> = app
            .world()
            .resource::<Messages<RequestTrade>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].target_char_id, 1337);
        assert!(app.world().get_entity(menu).is_err());
    }

    #[test]
    fn stale_trade_menu_cannot_send_while_session_open() {
        let mut app = App::new();
        app.add_message::<PartyInviteRequested>()
            .add_message::<GuildInviteRequested>()
            .add_message::<RequestTrade>()
            .insert_resource(ZoneSessionGeneration(3))
            .init_resource::<TradeSession>()
            .init_resource::<GuildUi>();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(77, "Alice".into());
        app.world_mut()
            .spawn((PlayerContextMenuRoot, ContextMenuTarget(1337)));
        let button = app
            .world_mut()
            .spawn(MenuAction::Trade)
            .observe(on_invite)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();
        assert!(app.world().resource::<Messages<RequestTrade>>().is_empty());
    }

    #[test]
    fn party_action_preserves_the_existing_command_payload() {
        let mut app = App::new();
        app.add_message::<PartyInviteRequested>()
            .add_message::<RequestTrade>()
            .add_message::<GuildInviteRequested>()
            .insert_resource(ZoneSessionGeneration(3))
            .init_resource::<TradeSession>()
            .init_resource::<GuildUi>();
        app.world_mut()
            .spawn((PlayerContextMenuRoot, ContextMenuTarget(1337)));
        let button = app
            .world_mut()
            .spawn(MenuAction::Party)
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
            .add_message::<RequestTrade>()
            .add_message::<GuildInviteRequested>()
            .insert_resource(ZoneSessionGeneration(3))
            .init_resource::<TradeSession>()
            .init_resource::<GuildUi>();
        app.world_mut()
            .spawn((PlayerContextMenuRoot, ContextMenuTarget(1337)));
        let button = app
            .world_mut()
            .spawn(MenuAction::Guild)
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
