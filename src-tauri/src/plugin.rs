use bevy::ecs::entity::Entity;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy::window::{Window, WindowResized, WindowResolution, WindowScaleFactorChanged};
use bevy_auto_plugin::modes::global::prelude::{auto_plugin, AutoPlugin};

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

pub fn configure_tauri_system_sets(app: &mut App) {
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
}

type WindowResizeSystemState = SystemState<(
    MessageWriter<'static, WindowResized>,
    Query<'static, 'static, (Entity, &'static mut Window)>,
)>;

type WindowFactorChangeSystemState = SystemState<(
    MessageWriter<'static, WindowResized>,
    MessageWriter<'static, WindowScaleFactorChanged>,
    Query<'static, 'static, (Entity, &'static mut Window)>,
)>;

pub fn handle_window_event(event: tauri::WindowEvent, app: &mut App) {
    match event {
        tauri::WindowEvent::Resized(size) => handle_window_resize(size, app),
        tauri::WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
            ..
        } => handle_window_factor_change(scale_factor, new_inner_size, app),
        _ => {}
    }
}

fn handle_window_resize(size: tauri::PhysicalSize<u32>, app: &mut App) {
    let mut system_state: WindowResizeSystemState = SystemState::new(app.world_mut());
    let (mut window_resized, mut window_query) = system_state.get_mut(app.world_mut());

    for (entity, mut window) in window_query.iter_mut() {
        window.resolution = WindowResolution::new(size.width, size.height);
        window_resized.write(WindowResized {
            window: entity,
            width: size.width as f32,
            height: size.height as f32,
        });
    }
}

fn handle_window_factor_change(
    scale_factor: f64,
    new_inner_size: tauri::PhysicalSize<u32>,
    app: &mut App,
) {
    let mut system_state: WindowFactorChangeSystemState = SystemState::new(app.world_mut());
    let (mut window_resized, mut scale_factor_changed, mut window_query) =
        system_state.get_mut(app.world_mut());

    for (entity, mut window) in window_query.iter_mut() {
        window.resolution = WindowResolution::new(new_inner_size.width, new_inner_size.height);
        scale_factor_changed.write(WindowScaleFactorChanged {
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
