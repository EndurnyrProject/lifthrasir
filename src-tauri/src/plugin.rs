use bevy::app::{App as BevyApp, AppExit, Plugin, PluginsState};
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Query, SystemState};
use bevy::prelude::MessageWriter;
use bevy::prelude::*;
use bevy::render::renderer::initialize_renderer;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
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

use super::bridge::{
    cleanup_stale_correlations, demux_tauri_events, emit_world_events,
    handle_create_character_request, handle_delete_character_request,
    handle_get_character_list_request, handle_get_hairstyles_request, handle_keyboard_input,
    handle_login_request, handle_mouse_click, handle_mouse_position,
    handle_select_character_request, handle_server_selection_request,
    write_character_creation_response, write_character_deletion_response,
    write_character_list_response, write_character_selection_response,
    write_login_failure_response, write_login_success_response, write_server_selection_response,
    AppBridge, CharacterCorrelation, CreateCharacterRequestedEvent, DeleteCharacterRequestedEvent,
    GetCharacterListRequestedEvent, GetHairstylesRequestedEvent, KeyboardInputEvent,
    LoginCorrelation, LoginRequestedEvent, MouseClickEvent, MousePositionEvent,
    PendingCharacterListSenders, PendingHairstyleSenders, SelectCharacterRequestedEvent,
    ServerCorrelation, ServerSelectionRequestedEvent, TauriEventReceiver, WorldEmitter,
};
use super::commands;
use game_engine::infrastructure::assets::SharedCompositeAssetSource;

/// Type alias for window resize event system state
type WindowResizeSystemState = SystemState<(
    MessageWriter<'static, WindowResized>,
    Query<'static, 'static, (Entity, &'static mut Window)>,
)>;

/// Type alias for window scale factor change event system state
type WindowFactorChangeSystemState = SystemState<(
    MessageWriter<'static, WindowResized>,
    MessageWriter<'static, WindowScaleFactorChanged>,
    Query<'static, 'static, (Entity, &'static mut Window)>,
)>;

/// Custom renderer plugin that creates the WGPU surface from Tauri window
struct CustomRendererPlugin {
    webview_window: WebviewWindow,
}

impl Plugin for CustomRendererPlugin {
    fn build(&self, app: &mut BevyApp) {
        // Create window wrapper and raw handle for Bevy's renderer
        let window_wrapper = WindowWrapper::new(self.webview_window.clone());
        let raw_handle = RawHandleWrapper::new(&window_wrapper).unwrap();
        let raw_handle_holder =
            RawHandleWrapperHolder(Arc::new(std::sync::Mutex::new(Some(raw_handle))));

        // Initialize renderer with new Bevy 0.17 API
        let render_resources = block_on(initialize_renderer(
            Backends::all(),
            Some(raw_handle_holder),
            &WgpuSettings::default(),
        ));

        app.add_plugins(RenderPlugin {
            render_creation: RenderCreation::Manual(render_resources),
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

        // Create SpritePngCache
        let sprite_png_cache = Arc::new(
            game_engine::infrastructure::sprite_png::SpritePngCache::new(
                sprite_renderer,
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
                        resolution: WindowResolution::new(1440, 1080),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .build()
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<RenderPlugin>()
                .disable::<bevy::prelude::ImagePlugin>()
                .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
                .disable::<bevy::core_pipeline::CorePipelinePlugin>()
                .disable::<bevy::sprite::SpritePlugin>()
                .disable::<bevy::sprite_render::SpriteRenderPlugin>()
                .disable::<bevy::text::TextPlugin>()
                .disable::<bevy::ui::UiPlugin>()
                .disable::<bevy::ui_render::UiRenderPlugin>()
                .disable::<bevy::pbr::PbrPlugin>()
                .disable::<bevy::gltf::GltfPlugin>()
                .disable::<bevy::audio::AudioPlugin>()
                .disable::<bevy::animation::AnimationPlugin>()
                .disable::<bevy::gizmos::GizmoPlugin>()
                .disable::<bevy::post_process::PostProcessPlugin>()
                .disable::<bevy::anti_alias::AntiAliasPlugin>(), // Re-added later with render pipeline
        );

        // Create AppBridge and event receiver
        let (app_bridge, tauri_rx) = AppBridge::new();

        app.insert_resource(app_bridge.clone());
        app.insert_resource(TauriEventReceiver(tauri_rx));
        app.insert_resource(LoginCorrelation::default());
        app.insert_resource(CharacterCorrelation::default());
        app.insert_resource(ServerCorrelation::default());
        app.insert_resource(PendingCharacterListSenders::default());
        app.insert_resource(PendingHairstyleSenders::default());

        // Register typed Bevy events for Tauri bridge
        app.add_message::<LoginRequestedEvent>()
            .add_message::<ServerSelectionRequestedEvent>()
            .add_message::<GetCharacterListRequestedEvent>()
            .add_message::<SelectCharacterRequestedEvent>()
            .add_message::<CreateCharacterRequestedEvent>()
            .add_message::<DeleteCharacterRequestedEvent>()
            .add_message::<GetHairstylesRequestedEvent>()
            .add_message::<KeyboardInputEvent>()
            .add_message::<MousePositionEvent>()
            .add_message::<MouseClickEvent>();

        // Add new event-driven system architecture
        // 1. demux_tauri_events: Reads from flume channel, emits typed events (runs first)
        // 2. Handler systems: Process typed events, emit game engine events (run after demux)
        // 3. Response writer systems: Capture game engine response events, send to UI (run after handlers)
        // 4. Cleanup system runs periodically to remove stale correlations
        app.add_systems(
            Update,
            (
                demux_tauri_events,
                (
                    handle_login_request,
                    handle_server_selection_request,
                    handle_get_character_list_request,
                    handle_select_character_request,
                    handle_create_character_request,
                    handle_delete_character_request,
                    handle_get_hairstyles_request,
                    handle_keyboard_input,
                    handle_mouse_position,
                    handle_mouse_click,
                ),
                (
                    write_login_success_response,
                    write_login_failure_response,
                    write_server_selection_response,
                    write_character_list_response,
                    write_character_selection_response,
                    write_character_creation_response,
                    write_character_deletion_response,
                ),
                cleanup_stale_correlations,
            )
                .chain(),
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
                commands::input::forward_mouse_click,
                commands::dev::open_devtools,
                commands::dev::close_devtools,
            ])
            .build(tauri::generate_context!())
            .expect("error while building tauri application");

        app.add_systems(
            Startup,
            (create_window_handle, register_composite_asset_source).chain(),
        );

        // Create and insert world emitter (for streaming zone/map status updates)
        let world_emitter = WorldEmitter::new(tauri_app.handle().clone());
        app.insert_resource(world_emitter);

        // Add world emitter system (streaming status updates to frontend)
        app.add_systems(Update, emit_world_events);

        app.insert_non_send_resource(tauri_app.handle().clone());
        app.insert_non_send_resource(tauri_app);
        app.set_runner(run_tauri_app);
    }
}

/// Custom runner that integrates Tauri's event loop with Bevy's update loop
#[allow(deprecated)]
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
    if app.plugins_state() != PluginsState::Cleaned && app.plugins_state() != PluginsState::Ready {
        tick_global_task_pools_on_main_thread();
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

        // Add custom renderer plugin that creates WGPU surface from Tauri window
        // This replaces the default RenderPlugin which we disabled in build()
        app.add_plugins(CustomRendererPlugin {
            webview_window: window,
        });

        // Add all rendering-related plugins that were disabled from DefaultPlugins
        // These must be added AFTER CustomRendererPlugin sets up the rendering resources
        let plugins = (
            bevy::prelude::ImagePlugin::default(),
            bevy::render::pipelined_rendering::PipelinedRenderingPlugin,
            bevy::core_pipeline::CorePipelinePlugin,
            bevy::sprite::SpritePlugin,
            bevy::sprite_render::SpriteRenderPlugin,
            bevy::text::TextPlugin,
            bevy::ui::UiPlugin,
            bevy::ui_render::UiRenderPlugin,
            bevy::pbr::PbrPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
            bevy::animation::AnimationPlugin,
            bevy::gizmos::GizmoPlugin,
            bevy::post_process::PostProcessPlugin,
            bevy::anti_alias::AntiAliasPlugin,
            game_engine::MapPlugin,
        );
        app.add_plugins(plugins);

        // Add game engine plugins AFTER rendering plugins are available
        // This ensures all required asset types are initialized
        app.add_plugins((
            bevy_tokio_tasks::TokioTasksPlugin::default(),
            game_engine::LifthrasirPlugin,
            game_engine::AssetsPlugin,
            game_engine::AudioPlugin,
            game_engine::AssetCatalogPlugin,
            game_engine::EntitySpawningPlugin,
            game_engine::CharacterDomainPlugin,
            game_engine::AuthenticationPlugin,
            game_engine::WorldPlugin,
            game_engine::BillboardPlugin,
            game_engine::MovementPlugin,
            game_engine::InputPlugin,
        ));

        // Add camera systems separately
        game_engine::LifthrasirPlugin::add_camera_systems(&mut app);

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
    let mut event_writer_system_state: WindowResizeSystemState = SystemState::new(app.world_mut());

    let (mut window_resized, mut window_query) = event_writer_system_state.get_mut(app.world_mut());

    for (entity, mut window) in window_query.iter_mut() {
        window.resolution = WindowResolution::new(size.width, size.height);
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
    let mut event_writer_system_state: WindowFactorChangeSystemState =
        SystemState::new(app.world_mut());

    let (mut window_resized, mut window_scale_factor_changed, mut window_query) =
        event_writer_system_state.get_mut(app.world_mut());

    for (entity, mut window) in window_query.iter_mut() {
        window.resolution = WindowResolution::new(new_inner_size.width, new_inner_size.height);
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
