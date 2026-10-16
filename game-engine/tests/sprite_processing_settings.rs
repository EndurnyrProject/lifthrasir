use bevy::prelude::*;
use game_engine::domain::audio::*;
use game_engine::domain::settings::{
    AudioConfig, GraphicsSettings, Keybinds, SettingsRuntimePlugin,
};
use lifthrasir_assets::animation::SpriteProcessingSystems;
use lifthrasir_assets::{SpriteProcessingSettings, Upscaling};

#[derive(Resource, Default)]
struct Observed(Option<Upscaling>);

fn observe_dispatch(settings: Res<SpriteProcessingSettings>, mut observed: ResMut<Observed>) {
    observed.0 = Some(settings.upscaling);
}

#[test]
fn client_preferences_reach_shared_processing_before_dispatch() {
    let mut app = App::new();
    app.init_resource::<GraphicsSettings>()
        .init_resource::<AudioConfig>()
        .init_resource::<Keybinds>()
        .init_resource::<AudioSettings>()
        .init_resource::<bevy_framepace::FramepaceSettings>()
        .init_resource::<SpriteProcessingSettings>()
        .init_resource::<Observed>()
        .add_message::<SetBgmVolumeEvent>()
        .add_message::<SetSfxVolumeEvent>()
        .add_message::<SetAmbienceVolumeEvent>()
        .add_message::<MuteBgmEvent>()
        .add_message::<MuteSfxEvent>()
        .add_message::<MuteAmbienceEvent>()
        .add_systems(
            Update,
            observe_dispatch.in_set(SpriteProcessingSystems::Dispatch),
        )
        .add_plugins(SettingsRuntimePlugin);
    app.world_mut().resource_mut::<GraphicsSettings>().upscaling = Upscaling::X3;
    app.update();
    assert_eq!(app.world().resource::<Observed>().0, Some(Upscaling::X3));
    app.world_mut().resource_mut::<GraphicsSettings>().upscaling = Upscaling::X2;
    app.update();
    assert_eq!(app.world().resource::<Observed>().0, Some(Upscaling::X2));
}
