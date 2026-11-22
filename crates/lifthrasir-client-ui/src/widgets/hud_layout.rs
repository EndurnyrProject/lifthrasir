//! Lets the player drag the hotbar, chat, minimap, status bar, status effects and
//! Homunculus plate around the HUD, and remembers where they were left.
//!
//! Each widget keeps its own anchor; the saved value is an offset from it, applied
//! as a [`UiTransform`] translation so the anchoring and layout stay untouched. A
//! drag that starts on something with its own pointer behavior (a button, a text
//! field, a hotbar slot) is left to it.

use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SaveSettings, SettingsGroup};
use bevy::text::EditableText;
use bevy::ui_widgets::Button;

use crate::widgets::hotbar::HotbarSlotUi;

/// Each movable widget's offset from its default spot, in UI pixels.
#[derive(Resource, SettingsGroup, Reflect, Default, Debug, Clone, PartialEq)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "hud_layout")]
pub(crate) struct HudLayout {
    pub(crate) hotbar: Vec2,
    pub(crate) chat: Vec2,
    pub(crate) minimap: Vec2,
    pub(crate) status_bar: Vec2,
    pub(crate) status_effects: Vec2,
    pub(crate) homunculus: Vec2,
}

impl HudLayout {
    pub(crate) fn offset(&self, widget: HudWidget) -> Vec2 {
        match widget {
            HudWidget::Hotbar => self.hotbar,
            HudWidget::Chat => self.chat,
            HudWidget::Minimap => self.minimap,
            HudWidget::StatusBar => self.status_bar,
            HudWidget::StatusEffects => self.status_effects,
            HudWidget::Homunculus => self.homunculus,
        }
    }

    fn offset_mut(&mut self, widget: HudWidget) -> &mut Vec2 {
        match widget {
            HudWidget::Hotbar => &mut self.hotbar,
            HudWidget::Chat => &mut self.chat,
            HudWidget::Minimap => &mut self.minimap,
            HudWidget::StatusBar => &mut self.status_bar,
            HudWidget::StatusEffects => &mut self.status_effects,
            HudWidget::Homunculus => &mut self.homunculus,
        }
    }
}

/// The node a drag moves, and which saved offset it follows.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudWidget {
    #[default]
    Hotbar,
    Chat,
    Minimap,
    StatusBar,
    StatusEffects,
    Homunculus,
}

pub struct HudLayoutPlugin;

impl Plugin for HudLayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudLayout>()
            .add_systems(Update, place_widgets);
    }
}

/// Makes the node a movable `widget`: drags on it or its children move it.
pub(crate) fn movable(widget: HudWidget) -> impl Scene {
    bsn! {
        widget
        UiTransform
        on(drag_widget)
        on(save_layout)
    }
}

/// [`movable`] for widgets spawned imperatively.
pub(crate) fn make_movable(commands: &mut Commands, entity: Entity, widget: HudWidget) {
    commands
        .entity(entity)
        .insert(widget)
        .observe(drag_widget)
        .observe(save_layout);
}

type OwnDrag = Or<(With<Button>, With<EditableText>, With<HotbarSlotUi>)>;

fn drag_widget(
    mut drag: On<PointerDrag>,
    widgets: Query<&HudWidget>,
    own_drag: Query<(), OwnDrag>,
    ui_scale: Res<UiScale>,
    mut layout: ResMut<HudLayout>,
) {
    let Ok(widget) = widgets.get(drag.entity) else {
        return;
    };
    drag.propagate(false);
    if own_drag.contains(drag.original_event_target()) {
        return;
    }
    *layout.offset_mut(*widget) += drag.delta / ui_scale.0;
}

fn save_layout(_: On<PointerDragEnd>, mut commands: Commands) {
    commands.queue(SaveSettings::IfChanged);
}

/// Moves each widget to its saved offset when it is spawned or the layout changes.
fn place_widgets(layout: Res<HudLayout>, mut widgets: Query<(Ref<HudWidget>, &mut UiTransform)>) {
    for (widget, mut transform) in &mut widgets {
        if !layout.is_changed() && !widget.is_added() {
            continue;
        }
        let offset = layout.offset(*widget);
        transform.translation = Val2::px(offset.x, offset.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawned_widget_takes_its_saved_offset() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(HudLayoutPlugin);
        app.world_mut().resource_mut::<HudLayout>().chat = Vec2::new(30.0, -12.0);
        let chat = app
            .world_mut()
            .spawn((HudWidget::Chat, UiTransform::default()))
            .id();
        let minimap = app
            .world_mut()
            .spawn((HudWidget::Minimap, UiTransform::default()))
            .id();

        app.update();

        let translation = |entity| app.world().get::<UiTransform>(entity).unwrap().translation;
        assert_eq!(translation(chat), Val2::px(30.0, -12.0));
        assert_eq!(translation(minimap), Val2::px(0.0, 0.0));
    }
}
