use bevy::prelude::*;
use lifthrasir_assets::{Gender, SpriteProcessingSettings, Upscaling};

#[test]
fn sprite_inputs_work_without_client_resources() {
    let mut app = App::new();
    app.init_resource::<SpriteProcessingSettings>();
    let entity = app.world_mut().spawn(Gender::Female).id();
    app.update();
    assert_eq!(app.world().get::<Gender>(entity), Some(&Gender::Female));
    assert_eq!(
        app.world().resource::<SpriteProcessingSettings>().upscaling,
        Upscaling::Off
    );
}

#[test]
fn serialized_gender_and_upscaling_values_remain_compatible() {
    assert_eq!(serde_json::to_string(&Gender::Female).unwrap(), "0");
    assert_eq!(serde_json::to_string(&Gender::Male).unwrap(), "1");
    assert_eq!(serde_json::from_str::<Gender>("0").unwrap(), Gender::Female);
    assert_eq!(serde_json::from_str::<Gender>("2").unwrap(), Gender::Male);
    let mut registry = bevy::reflect::TypeRegistry::default();
    registry.register::<Upscaling>();
    for (value, text, factor) in [
        (Upscaling::Off, "Off", None),
        (Upscaling::X2, "X2", Some(2)),
        (Upscaling::X3, "X3", Some(3)),
        (Upscaling::X4, "X4", Some(4)),
    ] {
        assert_eq!(ron::to_string(&value).unwrap(), text);
        assert_eq!(ron::from_str::<Upscaling>(text).unwrap(), value);
        assert_eq!(value.factor(), factor);
        let reflected = bevy::reflect::serde::TypedReflectSerializer::new(&value, &registry);
        assert_eq!(
            serde_json::to_value(reflected).unwrap(),
            serde_json::json!(text)
        );
    }
}
