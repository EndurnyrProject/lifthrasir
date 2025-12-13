use bevy::asset::{io::AssetSourceBuilder, io::AssetSourceId, AssetApp};
use bevy::prelude::*;
use bevy::remote::RemotePlugin;
use bevy::render::renderer::initialize_renderer;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::window::{
    RawHandleWrapper, RawHandleWrapperHolder, Window, WindowPlugin, WindowResolution, WindowWrapper,
};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{async_runtime::block_on, Manager, WebviewWindow};

use crate::bridge::{
    on_entity_name_added_to_hovered, on_hover_started_with_name, TauriEventReceiver, WorldEmitter,
};
use crate::config::{FRAMERATE_LIMIT, SPRITE_CACHE_CAPACITY};
use crate::plugin::{configure_tauri_system_sets, TauriIntegrationAutoPlugin};
use crate::resources::PreBevyResources;
use game_engine::core::state::GameState;
use game_engine::infrastructure::assets::{
    hierarchical_reader::HierarchicalAssetReader, sources::CompositeAssetSource,
    SharedCompositeAssetSource,
};
use game_engine::infrastructure::sprite_png::{SpritePngCache, SpriteRenderer};

pub fn create_bevy_app(
    app_handle: tauri::AppHandle,
    window: WebviewWindow,
    pre_bevy: &PreBevyResources,
) -> App {
    let mut app = App::new();

    let inner_size = window
        .inner_size()
        .expect("Failed to get window inner size");

    register_ro_asset_source(&mut app, pre_bevy.composite_source.clone());

    let render_plugin = create_render_plugin(&window);

    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::winit::WinitPlugin>()
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(inner_size.width, inner_size.height),
                    ..default()
                }),
                ..default()
            })
            .set(render_plugin),
    );

    app.insert_resource(ClearColor(Color::NONE));

    app.add_plugins(bevy_framepace::FramepacePlugin);
    app.world_mut()
        .resource_mut::<bevy_framepace::FramepaceSettings>()
        .limiter = bevy_framepace::Limiter::from_framerate(FRAMERATE_LIMIT);

    let sprite_renderer = Arc::new(SpriteRenderer::create_with_app(&mut app));
    let sprite_png_cache = Arc::new(
        SpritePngCache::new(sprite_renderer, SPRITE_CACHE_CAPACITY)
            .expect("Failed to create SpritePngCache"),
    );

    app_handle.manage(sprite_png_cache);

    app.insert_resource(pre_bevy.app_bridge.clone());
    app.insert_resource(TauriEventReceiver(pre_bevy.tauri_rx.clone()));

    let world_emitter = WorldEmitter::new(app_handle.clone());
    app.insert_resource(world_emitter);

    app.insert_non_send_resource(app_handle);

    configure_tauri_system_sets(&mut app);

    app.add_plugins(TauriIntegrationAutoPlugin);

    app.add_systems(OnEnter(GameState::InGame), create_window_handle);

    app.add_plugins(game_engine::MapPlugin);
    app.add_plugins(game_engine::CoreGamePlugins);

    app.add_observer(on_hover_started_with_name);
    app.add_observer(on_entity_name_added_to_hovered);

    #[cfg(debug_assertions)]
    add_debug_plugins(&mut app);

    app.finish();
    app.cleanup();

    info!("Bevy app initialized with full rendering pipeline");

    app
}

fn create_render_plugin(window: &WebviewWindow) -> RenderPlugin {
    let window_wrapper = WindowWrapper::new(window.clone());
    let raw_handle =
        RawHandleWrapper::new(&window_wrapper).expect("Failed to create raw handle wrapper");
    let raw_handle_holder = RawHandleWrapperHolder(Arc::new(Mutex::new(Some(raw_handle))));

    let render_resources = block_on(initialize_renderer(
        Backends::all(),
        Some(raw_handle_holder),
        &WgpuSettings::default(),
    ));

    RenderPlugin {
        render_creation: RenderCreation::Manual(render_resources),
        ..Default::default()
    }
}

fn register_ro_asset_source(app: &mut App, composite_source: Arc<RwLock<CompositeAssetSource>>) {
    app.register_asset_source(
        AssetSourceId::Name("ro".into()),
        AssetSourceBuilder::default().with_reader({
            let composite_clone = composite_source.clone();
            move || Box::new(HierarchicalAssetReader::new(composite_clone.clone()))
        }),
    );

    app.insert_resource(SharedCompositeAssetSource(composite_source));
}

#[cfg(debug_assertions)]
fn add_debug_plugins(app: &mut App) {
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default(),
        RemotePlugin::default(),
    ));
}

fn create_window_handle(
    mut commands: Commands,
    query: Query<(Entity, Option<&RawHandleWrapperHolder>)>,
    tauri_app: NonSend<tauri::AppHandle>,
) {
    let tauri_window = tauri_app.get_webview_window("main").unwrap();
    let window_wrapper = WindowWrapper::new(tauri_window);

    for (entity, handle_holder) in query.iter() {
        if let Ok(handle_wrapper) = RawHandleWrapper::new(&window_wrapper) {
            commands.entity(entity).insert(handle_wrapper.clone());

            if let Some(handle_holder) = handle_holder {
                *handle_holder.0.lock().unwrap() = Some(handle_wrapper);
            }
        }
    }
}
