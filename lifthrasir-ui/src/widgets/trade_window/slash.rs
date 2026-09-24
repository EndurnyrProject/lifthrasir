use bevy::prelude::*;
use game_engine::domain::entities::components::{EntityName, NetworkEntity};
use game_engine::domain::entities::types::ObjectType;
use game_engine::domain::trade::TradeSession;
use net_contract::commands::RequestTrade;
use net_contract::state::ZoneSession;

use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

/// Return the trimmed player name following `/trade`, or leave normal chat alone.
pub fn parse_trade_slash(input: &str) -> Option<String> {
    let (command, rest) = input.trim().split_once(char::is_whitespace)?;
    let name = rest.trim();
    (command == "/trade" && !name.is_empty()).then(|| name.to_string())
}

#[derive(Message, Debug, Clone)]
pub struct TradeSlashSubmitted(pub String);

pub(crate) fn resolve_nearby_pc<'a>(
    name: &str,
    self_gid: u32,
    units: impl Iterator<Item = (&'a NetworkEntity, &'a EntityName)>,
) -> Option<u32> {
    units
        .filter(|(net, _)| net.object_type == ObjectType::Pc && net.gid != self_gid)
        .find(|(_, entity_name)| entity_name.name.eq_ignore_ascii_case(name))
        .map(|(net, _)| net.gid)
}

#[expect(
    clippy::too_many_arguments,
    reason = "slash dispatch needs world and chat feedback"
)]
pub(crate) fn dispatch_trade_slash(
    mut submitted: MessageReader<TradeSlashSubmitted>,
    session: Res<TradeSession>,
    zone: Res<ZoneSession>,
    units: Query<(&NetworkEntity, &EntityName)>,
    mut requests: MessageWriter<RequestTrade>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for TradeSlashSubmitted(name) in submitted.read() {
        let result = if session.is_open() {
            Some("You are already trading.".to_string())
        } else if let Some(target_char_id) = resolve_nearby_pc(name, zone.char_id, units.iter()) {
            requests.write(RequestTrade { target_char_id });
            None
        } else {
            Some(format!("No player named {name} nearby."))
        };
        if let (Some(text), Ok(container)) = (result, container.single()) {
            append_colored_line(
                &mut commands,
                container,
                &text,
                theme::BAD,
                asset_server.load(theme::FONT_BODY),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_preserves_interior_spaces_and_rejects_missing_names() {
        assert_eq!(parse_trade_slash("/trade Bob"), Some("Bob".into()));
        assert_eq!(
            parse_trade_slash(" /trade  Bob Smith  "),
            Some("Bob Smith".into())
        );
        for input in ["/trade", "/trade   ", "/tradex Bob", "hello"] {
            assert_eq!(parse_trade_slash(input), None);
        }
    }

    #[test]
    fn resolver_matches_remote_pc_case_insensitively() {
        let local = NetworkEntity::new(1, 1, ObjectType::Pc);
        let mob = NetworkEntity::new(2, 2, ObjectType::Mob);
        let remote = NetworkEntity::new(3, 3, ObjectType::Pc);
        let names = [
            EntityName::new("Bob".into()),
            EntityName::new("Bob".into()),
            EntityName::new("Bob".into()),
        ];
        let units = [&local, &mob, &remote];
        let find = |name| resolve_nearby_pc(name, 1, units.iter().copied().zip(names.iter()));
        assert_eq!(find("bOb"), Some(3));
        assert_eq!(find("Alice"), None);
        assert_eq!(
            resolve_nearby_pc("Bob", 1, units[..2].iter().copied().zip(names[..2].iter())),
            None
        );
    }

    #[test]
    fn dispatch_sends_only_when_found_and_no_trade_is_open() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.init_resource::<TradeSession>()
            .insert_resource(ZoneSession {
                char_id: 1,
                ..default()
            });
        app.add_message::<TradeSlashSubmitted>()
            .add_message::<RequestTrade>();
        app.add_systems(Update, dispatch_trade_slash);
        app.world_mut().spawn(ChatHistory);
        app.world_mut().spawn((
            NetworkEntity::new(2, 2, ObjectType::Pc),
            EntityName::new("Bob".into()),
        ));
        app.world_mut()
            .write_message(TradeSlashSubmitted("Alice".into()));
        app.update();
        assert!(app.world().resource::<Messages<RequestTrade>>().is_empty());
        app.world_mut()
            .write_message(TradeSlashSubmitted("bOb".into()));
        app.update();
        let requests: Vec<_> = app
            .world()
            .resource::<Messages<RequestTrade>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].target_char_id, 2);
        app.world_mut()
            .resource_mut::<Messages<RequestTrade>>()
            .clear();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(2, "Bob".into());
        app.world_mut()
            .write_message(TradeSlashSubmitted("Bob".into()));
        app.update();
        assert!(app.world().resource::<Messages<RequestTrade>>().is_empty());
        let lines: Vec<_> = app
            .world_mut()
            .query::<(&Text, &TextColor)>()
            .iter(app.world())
            .map(|(t, c)| (t.0.clone(), c.0))
            .collect();
        assert!(lines.contains(&("No player named Alice nearby.".into(), theme::BAD)));
        assert!(lines.contains(&("You are already trading.".into(), theme::BAD)));
    }
}
