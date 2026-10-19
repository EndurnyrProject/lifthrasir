//! Navigation slash-command parsing and dispatch.

use bevy::prelude::*;
use net_contract::commands::{NavigationCancelRequested, NavigationRequested};
use net_contract::dto::NavigationTarget;

/// A recognized navigation slash command.
#[derive(Message, Debug, Clone, PartialEq)]
pub enum NaviSlash {
    To(NavigationTarget),
    Cancel,
}

/// Parse one chat line into a navigation slash command.
pub fn parse_navi_slash(input: &str) -> Option<NaviSlash> {
    let mut args = input.split_whitespace();
    (args.next()? == "/navi").then_some(())?;
    match (args.next(), args.next(), args.next(), args.next()) {
        (Some("cancel"), None, None, None) => Some(NaviSlash::Cancel),
        (Some(map), None, None, None) => Some(NaviSlash::To(NavigationTarget::Map(map.into()))),
        (Some(map), Some(x), Some(y), None) => Some(NaviSlash::To(NavigationTarget::Coord {
            map: map.into(),
            x: x.parse().ok()?,
            y: y.parse().ok()?,
        })),
        _ => None,
    }
}

/// Turn each parsed navigation slash into its matching outbound command.
pub fn dispatch_navi_slash(
    mut submitted: MessageReader<NaviSlash>,
    mut requested: MessageWriter<NavigationRequested>,
    mut cancelled: MessageWriter<NavigationCancelRequested>,
) {
    for slash in submitted.read() {
        match slash {
            NaviSlash::To(target) => {
                requested.write(NavigationRequested {
                    target: target.clone(),
                    flag: 0,
                    hide_window: false,
                });
            }
            NaviSlash::Cancel => {
                cancelled.write(NavigationCancelRequested);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_map_target() {
        assert_eq!(
            parse_navi_slash("/navi geffen"),
            Some(NaviSlash::To(NavigationTarget::Map("geffen".into())))
        );
    }

    #[test]
    fn parses_coordinate_target() {
        assert_eq!(
            parse_navi_slash("/navi prontera 150 99"),
            Some(NaviSlash::To(NavigationTarget::Coord {
                map: "prontera".into(),
                x: 150,
                y: 99,
            }))
        );
    }

    #[test]
    fn parses_cancel() {
        assert_eq!(parse_navi_slash("/navi cancel"), Some(NaviSlash::Cancel));
    }

    #[test]
    fn unrecognized_input_falls_through() {
        assert_eq!(parse_navi_slash("/navi"), None);
        assert_eq!(parse_navi_slash("/navigate somewhere"), None);
        assert_eq!(parse_navi_slash("/navix"), None);
        assert_eq!(parse_navi_slash("/navi geffen x 99"), None);
        assert_eq!(parse_navi_slash("/navi geffen 65536 99"), None);
        assert_eq!(parse_navi_slash("/navi geffen 150 99 extra"), None);
    }

    fn requests(app: &App) -> Vec<NavigationRequested> {
        let messages = app.world().resource::<Messages<NavigationRequested>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn cancels(app: &App) -> Vec<NavigationCancelRequested> {
        let messages = app
            .world()
            .resource::<Messages<NavigationCancelRequested>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn dispatches_requests_with_default_options_and_cancels() {
        let mut app = App::new();
        app.add_message::<NaviSlash>()
            .add_message::<NavigationRequested>()
            .add_message::<NavigationCancelRequested>()
            .add_systems(Update, dispatch_navi_slash);
        app.world_mut()
            .write_message(NaviSlash::To(NavigationTarget::Map("geffen".into())));
        app.world_mut().write_message(NaviSlash::Cancel);
        app.update();

        let written = requests(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target, NavigationTarget::Map("geffen".into()));
        assert_eq!(written[0].flag, 0);
        assert!(!written[0].hide_window);
        assert_eq!(cancels(&app).len(), 1);
    }
}
