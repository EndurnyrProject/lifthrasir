use bevy::{
    input::InputPlugin,
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    text::EditableText,
};
use lifthrasir_ui::{
    UiPrimitivesPlugin,
    theme::{
        EMERALD,
        feathers_theme::{TOKEN_ACCENT, assign_token, norse_theme},
    },
};

#[test]
fn shared_setup_preserves_host_theme_and_makes_text_fields_focusable() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<bevy::shader::Shader>();
    let mut theme = norse_theme();
    assign_token(&mut theme.0, TOKEN_ACCENT, EMERALD);
    app.insert_resource(theme).add_plugins(UiPrimitivesPlugin);
    let field = app.world_mut().spawn(EditableText::new("")).id();
    assert_eq!(app.world().get::<TabIndex>(field), Some(&TabIndex(0)));
    assert!(app.world().contains_resource::<InputFocus>());
    assert_eq!(
        app.world()
            .resource::<bevy_feathers::theme::UiTheme>()
            .color(&TOKEN_ACCENT),
        EMERALD
    );
    app.update();
}
