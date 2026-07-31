use bevy::{
    asset::{AssetApp, AssetPlugin, AssetServer},
    image::{CompressedImageFormats, Image, ImageLoader, ImagePlugin},
    prelude::*,
    render::render_resource::TextureFormat,
};
use std::time::{Duration, Instant};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

#[test]
fn loads_zstd_ktx2_fixture() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: FIXTURES.to_string(),
            ..default()
        },
        ImagePlugin::default(),
    ));
    app.register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));

    let handle: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .load("tex/mip_probe.ktx2");

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        let state = app.world().resource::<AssetServer>().load_state(&handle);
        assert!(!state.is_failed(), "KTX2 fixture failed to load: {state:?}");
        if state.is_loaded() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "KTX2 fixture never finished loading"
        );
        std::thread::sleep(Duration::from_millis(2));
    }

    let image = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .expect("loaded KTX2 image");
    assert_eq!(image.texture_descriptor.size.width, 4);
    assert_eq!(image.texture_descriptor.size.height, 4);
    assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 1);
    assert_eq!(image.texture_descriptor.mip_level_count, 3);
    assert_eq!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb
    );
}
