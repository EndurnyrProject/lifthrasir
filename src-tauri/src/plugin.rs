use bevy::app::{App as BevyApp, AppExit, Plugin, PluginsState};
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventWriter;
use bevy::ecs::system::{Query, SystemState};
use bevy::prelude::*;
use bevy::render::renderer::{initialize_renderer, RenderInstance, WgpuWrapper};
use bevy::render::settings::{RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::tasks::tick_global_task_pools_on_main_thread;
use bevy::window::{
    RawHandleWrapper, RawHandleWrapperHolder, Window, WindowPlugin, WindowResized,
    WindowResolution, WindowScaleFactorChanged, WindowWrapper,
};
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;
use tauri::{async_runtime::block_on, Manager, RunEvent, WebviewWindow};
use wgpu::RequestAdapterOptions;

use super::bridge::{
    translate_tauri_events, write_character_creation_response, write_character_deletion_response,
    write_character_list_response, write_character_selection_response,
    write_login_failure_response, write_login_success_response, write_server_selection_response,
    zone_status_event_emitter, AppBridge, PendingSenders, TauriEventEmitter, TauriEventReceiver,
};
use super::commands;
use game_engine::infrastructure::assets::SharedCompositeAssetSource;

/// Custom renderer plugin that creates the WGPU surface from Tauri window
struct CustomRendererPlugin {
    webview_window: WebviewWindow,
}

impl Plugin for CustomRendererPlugin {
    fn build(&self, app: &mut BevyApp) {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(&self.webview_window).unwrap();

        let (device, queue, adapter_info, adapter) = block_on(initialize_renderer(
            &instance,
            &WgpuSettings::default(),
            &RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            },
        ));

        app.add_plugins(RenderPlugin {
            render_creation: RenderCreation::Manual(bevy::render::settings::RenderResources(
                device,
                queue,
                adapter_info,
                adapter,
                RenderInstance(Arc::new(WgpuWrapper::new(instance))),
            )),
            ..Default::default()
        });
    }
}

/// System that creates window handle from Tauri window
fn create_window_handle(
    mut commands: Commands,
    query: Query<(Entity, Option<&'static RawHandleWrapperHolder>)>,
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

/// System that registers SharedCompositeAssetSource with Tauri
/// Runs after RoAssetsPlugin has created the resource
fn register_composite_asset_source(
    tauri_app: NonSend<tauri::AppHandle>,
    shared_composite: Option<Res<SharedCompositeAssetSource>>,
) {
    if let Some(composite) = shared_composite {
        let composite_arc = composite.0.clone();
        tauri_app.manage(composite_arc);
    }
}

/// Main Tauri integration plugin
pub struct TauriIntegrationPlugin;

impl Plugin for TauriIntegrationPlugin {
    fn build(&self, app: &mut BevyApp) {
        let ro_asset_source_plugin =
            game_engine::infrastructure::assets::ro_assets_plugin::RoAssetsPlugin::with_unified_source();
        app.add_plugins(ro_asset_source_plugin);

        // Load asset configuration for sprite renderer
        let config_path = "assets/loader.data.toml";
        let config: game_engine::infrastructure::assets::AssetConfig = {
            let config_content =
                std::fs::read_to_string(config_path).expect("Failed to read asset config");
            toml::from_str(&config_content).expect("Failed to parse asset config")
        };

        // Create HierarchicalAssetManager for sprite rendering
        let asset_manager =
            game_engine::infrastructure::assets::HierarchicalAssetManager::from_config(&config)
                .expect("Failed to create HierarchicalAssetManager");

        // Create SpriteRenderer
        let sprite_renderer =
            Arc::new(game_engine::infrastructure::sprite_png::SpriteRenderer::new(asset_manager));

        // Set up cache directory
        let cache_dir = std::env::current_dir()
            .unwrap()
            .join(".cache")
            .join("sprites");

        // Create SpritePngCache
        let sprite_png_cache = Arc::new(
            game_engine::infrastructure::sprite_png::SpritePngCache::new(
                sprite_renderer,
                cache_dir,
                100, // 100 sprites in memory cache
            )
            .expect("Failed to create SpritePngCache"),
        );

        // Add DefaultPlugins with customizations
        // - WindowPlugin is customized to set primary window
        // - WinitPlugin is disabled because Tauri manages the window and event loop
        // - All rendering-related plugins are disabled and added later in handle_ready_event
        //   after our CustomRendererPlugin creates the WGPU surface from Tauri window
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (1440.0, 1080.0).into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .build()
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<RenderPlugin>()
                .disable::<bevy::render::texture::ImagePlugin>()
                .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
                .disable::<bevy::core_pipeline::CorePipelinePlugin>()
                .disable::<bevy::sprite::SpritePlugin>()
                .disable::<bevy::text::TextPlugin>()
                .disable::<bevy::ui::UiPlugin>()
                .disable::<bevy::pbr::PbrPlugin>()
                .disable::<bevy::gltf::GltfPlugin>()
                .disable::<bevy::audio::AudioPlugin>()
                .disable::<bevy::animation::AnimationPlugin>()
                .disable::<bevy::gizmos::GizmoPlugin>(), // Re-added later with render pipeline
        );

        // Add game engine plugins (AFTER core Bevy plugins are set up)
        // This ensures StatesPlugin is available before game plugins that use states
        app.add_plugins((
            bevy_tokio_tasks::TokioTasksPlugin::default(),
            game_engine::LifthrasirPlugin,
            game_engine::AssetsPlugin,
            game_engine::AudioPlugin,     // Audio system with BGM support and crossfading
            game_engine::AssetCatalogPlugin,
            game_engine::CharacterDomainPlugin, // Includes UnifiedCharacterEntityPlugin with 3D sprite hierarchy
            game_engine::AuthenticationPlugin,
            game_engine::WorldPlugin,
            game_engine::BillboardPlugin, // 3D billboard rendering infrastructure
            game_engine::InputPlugin,     // Input handling (cursor, clicks, terrain cursor)
        ));

        // Create AppBridge and event receiver
        let (app_bridge, tauri_rx) = AppBridge::new();

        app.insert_resource(app_bridge.clone());
        app.insert_resource(TauriEventReceiver(tauri_rx));
        app.insert_resource(PendingSenders::default());

        // Add event translation systems
        // Split into multiple add_systems to avoid tuple size limits
        app.add_systems(Update, translate_tauri_events);
        app.add_systems(
            Update,
            (
                write_login_success_response,
                write_login_failure_response,
                write_server_selection_response,
                write_character_list_response,
                write_character_selection_response,
                write_character_creation_response,
                write_character_deletion_response,
            )
                .after(translate_tauri_events),
        );

        // Set up Tauri app with custom runner
        let tauri_app = tauri::Builder::default()
            .manage(app_bridge)
            .manage(sprite_png_cache)
            .invoke_handler(tauri::generate_handler![
                commands::auth::login,
                commands::assets::get_asset,
                commands::server_selection::select_server,
                commands::character_selection::get_character_list,
                commands::character_selection::select_character,
                commands::character_selection::create_character,
                commands::character_selection::delete_character,
                commands::character_creation::get_hairstyles,
                commands::sprite_png::get_sprite_png,
                commands::sprite_png::preload_sprite_batch,
                commands::sprite_png::clear_sprite_cache,
                commands::zone_status::get_zone_status,
                commands::input::forward_keyboard_input,
                commands::input::forward_mouse_position,
            ])
            .build(tauri::generate_context!())
            .expect("error while building tauri application");

        app.add_systems(
            Startup,
            (create_window_handle, register_composite_asset_source).chain(),
        );

        // Create and insert TauriEventEmitter for zone status events
        let event_emitter = TauriEventEmitter::new(tauri_app.handle().clone());
        app.insert_non_send_resource(event_emitter);

        // Add zone status event emitter system
        app.add_systems(Update, zone_status_event_emitter);

        app.insert_non_send_resource(tauri_app.handle().clone());
        app.insert_non_send_resource(tauri_app);
        app.set_runner(run_tauri_app);
    }
}

/// Custom runner that integrates Tauri's event loop with Bevy's update loop
fn run_tauri_app(app: App) -> AppExit {
    let app = Rc::new(RefCell::new(app));
    let mut tauri_app = app
        .borrow_mut()
        .world_mut()
        .remove_non_send_resource::<tauri::App>()
        .unwrap();

    loop {
        let app_clone = app.clone();
        tauri_app.run_iteration(move |app_handle, event: RunEvent| {
            handle_tauri_events(app_handle, event, app_clone.borrow_mut());
        });

        if tauri_app.webview_windows().is_empty() {
            tauri_app.cleanup_before_exit();
            break;
        }

        app.borrow_mut().update();
    }

    AppExit::Success
}

/// Handle Tauri events and integrate with Bevy
fn handle_tauri_events(
    app_handle: &tauri::AppHandle,
    event: RunEvent,
    mut app: RefMut<'_, BevyApp>,
) {
    if app.plugins_state() != PluginsState::Cleaned {
        if app.plugins_state() != PluginsState::Ready {
            tick_global_task_pools_on_main_thread();
        }
    }

    match event {
        tauri::RunEvent::Ready => handle_ready_event(app_handle, app),
        tauri::RunEvent::WindowEvent { event, .. } => handle_window_event(event, app),
        _ => (),
    }
}

/// Handle Tauri ready event - set up custom rendering plugin and rendering-related plugins
fn handle_ready_event(app_handle: &tauri::AppHandle, mut app: RefMut<'_, BevyApp>) {
    if app.plugins_state() != PluginsState::Cleaned {
        let window = app_handle.get_webview_window("main").unwrap();

        // Open devtools in debug mode for frontend debugging
        #[cfg(debug_assertions)]
        {
            window.open_devtools();
            info!("DevTools opened for debugging");
        }

        // Add custom renderer plugin that creates WGPU surface from Tauri window
        // This replaces the default RenderPlugin which we disabled in build()
        app.add_plugins(CustomRendererPlugin {
            webview_window: window,
        });

        // Add all rendering-related plugins that were disabled from DefaultPlugins
        // These must be added AFTER CustomRendererPlugin sets up the rendering resources
        app.add_plugins((
            bevy::render::texture::ImagePlugin::default(),
            bevy::render::pipelined_rendering::PipelinedRenderingPlugin::default(),
            bevy::core_pipeline::CorePipelinePlugin::default(),
            bevy::sprite::SpritePlugin::default(),
            bevy::text::TextPlugin::default(),
            bevy::ui::UiPlugin::default(),
            bevy::pbr::PbrPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
            // NOTE: bevy::audio::AudioPlugin is NOT added here - we use bevy_kira_audio instead
            // (added via game_engine::AudioPlugin on line 172)
            bevy::animation::AnimationPlugin::default(),
            bevy::gizmos::GizmoPlugin::default(),
            game_engine::MapPlugin,
        ));

        // Wait for all plugins to be ready
        while app.plugins_state() != PluginsState::Ready {
            tick_global_task_pools_on_main_thread();
        }

        app.finish();
        app.cleanup();
    }
}

/// Handle window events (resize, scale factor changes)
fn handle_window_event(event: tauri::WindowEvent, app: RefMut<'_, BevyApp>) {
    match event {
        tauri::WindowEvent::Resized(size) => handle_window_resize(size, app),
        tauri::WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
            ..
        } => handle_window_factor_change(scale_factor, new_inner_size, app),
        _ => (),
    }
}

/// Handle window resize events
fn handle_window_resize(size: tauri::PhysicalSize<u32>, mut app: RefMut<'_, BevyApp>) {
    let mut event_writer_system_state: SystemState<(
        EventWriter<WindowResized>,
        Query<(Entity, &mut Window)>,
    )> = SystemState::new(app.world_mut());

    let (mut window_resized, mut window_query) = event_writer_system_state.get_mut(app.world_mut());

    for (entity, mut window) in window_query.iter_mut() {
        window.resolution = WindowResolution::new(size.width as f32, size.height as f32);
        window_resized.write(WindowResized {
            window: entity,
            width: size.width as f32,
            height: size.height as f32,
        });
    }
}

/// Handle window scale factor changes
fn handle_window_factor_change(
    scale_factor: f64,
    new_inner_size: tauri::PhysicalSize<u32>,
    mut app: RefMut<'_, BevyApp>,
) {
    let mut event_writer_system_state: SystemState<(
        EventWriter<WindowResized>,
        EventWriter<WindowScaleFactorChanged>,
        Query<(Entity, &mut Window)>,
    )> = SystemState::new(app.world_mut());

    let (mut window_resized, mut window_scale_factor_changed, mut window_query) =
        event_writer_system_state.get_mut(app.world_mut());

    for (entity, mut window) in window_query.iter_mut() {
        window.resolution =
            WindowResolution::new(new_inner_size.width as f32, new_inner_size.height as f32);
        window_scale_factor_changed.write(WindowScaleFactorChanged {
            window: entity,
            scale_factor,
        });
        window_resized.write(WindowResized {
            window: entity,
            width: new_inner_size.width as f32,
            height: new_inner_size.height as f32,
        });
    }
}
