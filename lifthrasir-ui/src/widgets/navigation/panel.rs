use bevy::prelude::*;
use game_engine::domain::world::navigation::ActiveRoute;

use crate::{
    theme,
    widgets::hotbar::{HOTBAR_BOTTOM, HOTBAR_HEIGHT},
};

const PANEL_Z: i32 = 100;
const HOTBAR_GAP: f32 = 8.0;

#[derive(Component)]
pub(super) struct NavigationPanel;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigationPanelText {
    Destination,
    Progress,
    Portal,
    NextMap,
}

pub(crate) fn spawn_navigation_panel(commands: &mut Commands, parent: Entity, font: Handle<Font>) {
    let panel = commands
        .spawn((
            NavigationPanel,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(HOTBAR_BOTTOM + HOTBAR_HEIGHT + HOTBAR_GAP),
                width: Val::Px(220.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::EMERALD_DEEP),
            GlobalZIndex(PANEL_Z),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    commands.spawn((
        theme::label("", font.clone(), 12.0, theme::TEXT),
        Node::default(),
        NavigationPanelText::Destination,
        ChildOf(panel),
    ));
    commands.spawn((
        theme::label("", font.clone(), 11.0, theme::TEXT_DIM),
        Node::default(),
        NavigationPanelText::Progress,
        ChildOf(panel),
    ));
    commands.spawn((
        theme::label("", font.clone(), 11.0, theme::GOLD),
        Node {
            display: Display::None,
            ..default()
        },
        NavigationPanelText::Portal,
        ChildOf(panel),
    ));
    commands.spawn((
        theme::label("", font, 11.0, theme::TEXT),
        Node {
            display: Display::None,
            ..default()
        },
        NavigationPanelText::NextMap,
        ChildOf(panel),
    ));
}

pub(super) fn sync_navigation_panel(
    active_route: Res<ActiveRoute>,
    mut panels: Query<&mut Visibility, With<NavigationPanel>>,
    mut texts: Query<(&mut Text, &mut Node, &NavigationPanelText)>,
) {
    let Some(route) = active_route.0.as_ref() else {
        set_panel_visibility(&mut panels, false);
        return;
    };

    let Some(leg) = route.current_leg() else {
        set_panel_visibility(&mut panels, false);
        return;
    };

    set_panel_visibility(&mut panels, !route.hide_window);
    let (current, total) = route.leg_progress();
    for (mut text, mut node, kind) in &mut texts {
        match kind {
            NavigationPanelText::Destination => {
                set_text(
                    &mut text,
                    format!(
                        "{} ({}, {})",
                        route.destination.map, route.destination.x, route.destination.y
                    ),
                );
            }
            NavigationPanelText::Progress => set_text(&mut text, format!("{current}/{total}")),
            NavigationPanelText::Portal => {
                set_optional_line(&mut text, &mut node, "Portal", leg.exit_portal.as_deref());
            }
            NavigationPanelText::NextMap => {
                set_optional_line(&mut text, &mut node, "Next", leg.next_map.as_deref());
            }
        }
    }
}

fn set_panel_visibility(panels: &mut Query<&mut Visibility, With<NavigationPanel>>, visible: bool) {
    let wanted = if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in panels {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

fn set_optional_line(text: &mut Text, node: &mut Node, label: &str, value: Option<&str>) {
    let display = if value.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
    set_text(
        text,
        value.map_or_else(String::new, |value| format!("{label}: {value}")),
    );
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        *text = Text::new(value);
    }
}

#[cfg(test)]
mod tests {
    use game_engine::domain::world::navigation::{ActiveRoute, Route};
    use net_contract::dto::{RouteDestination, RouteLeg};

    use super::*;

    fn panel_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ActiveRoute>()
            .add_systems(Startup, spawn_panel_for_test)
            .add_systems(Update, sync_navigation_panel);
        app
    }

    fn spawn_panel_for_test(mut commands: Commands) {
        let parent = commands.spawn_empty().id();
        spawn_navigation_panel(&mut commands, parent, Handle::default());
    }

    fn route(hide_window: bool) -> Route {
        Route {
            generation: 1,
            legs: vec![
                RouteLeg {
                    map: "prontera".into(),
                    cells: vec![(1, 1)],
                    exit_portal: Some("North Gate".into()),
                    next_map: Some("geffen".into()),
                },
                RouteLeg {
                    map: "geffen".into(),
                    cells: vec![(2, 2)],
                    exit_portal: Some("West Gate".into()),
                    next_map: Some("payon".into()),
                },
                RouteLeg {
                    map: "payon".into(),
                    cells: vec![(3, 3)],
                    exit_portal: None,
                    next_map: None,
                },
            ],
            current: 1,
            destination: RouteDestination {
                map: "payon".into(),
                x: 120,
                y: 44,
            },
            hide_window,
        }
    }

    fn panel(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<NavigationPanel>>()
            .single(app.world())
            .unwrap()
    }

    fn text(app: &mut App, kind: NavigationPanelText) -> String {
        app.world_mut()
            .query::<(&Text, &NavigationPanelText)>()
            .iter(app.world())
            .find_map(|(text, marker)| (*marker == kind).then(|| text.0.clone()))
            .unwrap()
    }

    fn line_display(app: &mut App, kind: NavigationPanelText) -> Display {
        app.world_mut()
            .query::<(&Node, &NavigationPanelText)>()
            .iter(app.world())
            .find_map(|(node, marker)| (*marker == kind).then_some(node.display))
            .unwrap()
    }

    #[test]
    fn panel_is_hidden_without_an_active_route() {
        let mut app = panel_app();

        app.update();

        let panel = panel(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn panel_is_hidden_when_the_route_hides_its_window() {
        let mut app = panel_app();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route(true));

        app.update();

        let panel = panel(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn mid_route_shows_destination_progress_and_next_portal() {
        let mut app = panel_app();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route(false));

        app.update();

        let panel = panel(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            text(&mut app, NavigationPanelText::Destination),
            "payon (120, 44)"
        );
        assert_eq!(text(&mut app, NavigationPanelText::Progress), "2/3");
        assert_eq!(
            text(&mut app, NavigationPanelText::Portal),
            "Portal: West Gate"
        );
        assert_eq!(text(&mut app, NavigationPanelText::NextMap), "Next: payon");
    }

    #[test]
    fn final_leg_omits_portal_and_next_map_lines() {
        let mut app = panel_app();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route(false));

        app.update();

        assert_eq!(
            text(&mut app, NavigationPanelText::Portal),
            "Portal: West Gate"
        );
        assert_eq!(text(&mut app, NavigationPanelText::NextMap), "Next: payon");
        assert_eq!(
            line_display(&mut app, NavigationPanelText::Portal),
            Display::Flex
        );
        assert_eq!(
            line_display(&mut app, NavigationPanelText::NextMap),
            Display::Flex
        );

        let mut final_leg = route(false);
        final_leg.current = 2;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(final_leg);
        app.update();

        assert_eq!(
            line_display(&mut app, NavigationPanelText::Portal),
            Display::None
        );
        assert_eq!(
            line_display(&mut app, NavigationPanelText::NextMap),
            Display::None
        );
    }

    #[test]
    fn panel_is_hidden_when_the_route_has_no_current_leg() {
        let mut app = panel_app();
        let mut empty_route = route(false);
        empty_route.legs.clear();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(empty_route);

        app.update();

        let panel = panel(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn panel_content_updates_when_the_route_changes() {
        let mut app = panel_app();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route(false));
        app.update();

        let mut replacement = route(false);
        replacement.generation = 2;
        replacement.destination = RouteDestination {
            map: "morocc".into(),
            x: 8,
            y: 16,
        };
        replacement.current = 0;
        replacement.legs[0].exit_portal = Some("South Gate".into());
        replacement.legs[0].next_map = Some("izlude".into());
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(replacement);

        app.update();

        assert_eq!(
            text(&mut app, NavigationPanelText::Destination),
            "morocc (8, 16)"
        );
        assert_eq!(text(&mut app, NavigationPanelText::Progress), "1/3");
        assert_eq!(
            text(&mut app, NavigationPanelText::Portal),
            "Portal: South Gate"
        );
        assert_eq!(text(&mut app, NavigationPanelText::NextMap), "Next: izlude");
    }
}
