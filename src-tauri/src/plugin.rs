use bevy::app::{App as BevyApp, AppExit, Plugin, PluginsState};
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Query, SystemState};
use bevy::prelude::MessageWriter;
use bevy::prelude::*;
use bevy::remote::RemotePlugin;
use bevy::render::renderer::initialize_renderer;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::tasks::tick_global_task_pools_on_main_thread;
use bevy::window::{
    RawHandleWrapper, RawHandleWrapperHolder, Window, WindowPlugin, WindowResized,
    WindowResolution, WindowScaleFactorChanged, WindowWrapper,
};
use bevy_auto_plugin::modes::global::prelude::{auto_plugin, AutoPlugin};
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;
use tauri::{async_runtime::block_on, Manager, RunEvent, WebviewWindow};

use super::bridge::{on_entity_name_added_to_hovered, AppBridge, TauriEventReceiver, WorldEmitter};
use super::commands;
use game_engine::infrastructure::assets::SharedCompositeAssetSource;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TauriSystems {
    Demux,
    Handlers,
    ResponseWriters,
    Emitters,
    Cleanup,
}

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct TauriIntegrationAutoPlugin;

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

        // Create SpriteRenderer integrated with Bevy's asset system
        // This sets up a channel-based communication system between Tauri commands and Bevy's ECS
        let sprite_renderer =
            Arc::new(game_engine::infrastructure::sprite_png::SpriteRenderer::create_with_app(app));

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
        // - LogPlugin is customized to output to both console and file
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
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter: "wgpu=error,bevy_render=info,bevy_ecs=warn".to_string(),
                    custom_layer: |_app| {
                        use std::{fs::File, sync::Arc};

                        // Create logs directory if it doesn't exist
                        let _ = std::fs::create_dir_all("logs");

                        // Create file writer for all logs
                        let file = Arc::new(File::create("logs/lifthrasir.log").ok()?);

                        // Write only to file (console logging handled by default fmt_layer)
                        Some(Box::new(
                            bevy::log::tracing_subscriber::fmt::layer().with_writer(file),
                        ))
                    },
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

        // Create AppBridge and event receiver (runtime values - must be manual)
        let (app_bridge, tauri_rx) = AppBridge::new();
        app.insert_resource(app_bridge.clone());
        app.insert_resource(TauriEventReceiver(tauri_rx));

        // Configure TauriSystems ordering: Demux -> Handlers -> ResponseWriters -> Emitters -> Cleanup
        app.configure_sets(
            Update,
            (
                TauriSystems::Demux,
                TauriSystems::Handlers,
                TauriSystems::ResponseWriters,
                TauriSystems::Emitters,
                TauriSystems::Cleanup,
            )
                .chain(),
        );

        // Add auto-plugin for events, resources, and systems
        app.add_plugins(TauriIntegrationAutoPlugin);

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
                commands::character_status::get_character_status,
                commands::sprite_png::get_sprite_png,
                commands::sprite_png::preload_sprite_batch,
                commands::sprite_png::clear_sprite_cache,
                commands::zone_status::get_zone_status,
                commands::input::forward_keyboard_input,
                commands::input::forward_mouse_position,
                commands::input::forward_mouse_click,
                commands::input::forward_camera_rotation,
                commands::chat::send_chat_message,
                commands::dev::open_devtools,
                commands::dev::close_devtools,
                commands::dev::refresh_window,
            ])
            .build(tauri::generate_context!())
            .expect("error while building tauri application");

        app.add_systems(
            Startup,
            (create_window_handle, register_composite_asset_source).chain(),
        );

        // Create and insert world emitter (for streaming zone/map status updates)
        // WorldEmitter requires runtime tauri AppHandle - must be manual
        let world_emitter = WorldEmitter::new(tauri_app.handle().clone());
        app.insert_resource(world_emitter);

        // Observer triggers when EntityName component is added to hovered entities
        // This solves the race condition where names arrive from server after hover detection
        app.add_observer(on_entity_name_added_to_hovered);

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

        // Add framepace plugin for 60 FPS limiting
        app.add_plugins(bevy_framepace::FramepacePlugin);

        // Configure framepace to 60 FPS immediately after plugin is added
        app.world_mut()
            .resource_mut::<bevy_framepace::FramepaceSettings>()
            .limiter = bevy_framepace::Limiter::from_framerate(60.0);

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

        // Add Bevy diagnostic plugins for development
        #[cfg(debug_assertions)]
        {
            app.add_plugins((
                bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
                bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default(),
                RemotePlugin::default(),
            ));
        }

        // Add game engine plugins AFTER rendering plugins are available
        // This ensures all required asset types are initialized
        app.add_plugins(game_engine::CoreGamePlugins);

        // Add camera systems separately
        game_engine::LifthrasirPlugin::add_camera_systems(&mut app);

        // WORKAROUND: macOS WebKit transparent window bug
        // Toggling decorations forces a repaint that makes content visible
        // See: https://github.com/tauri-apps/tauri/issues/8255
        // #[cfg(target_os = "macos")]
        // let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);

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
