#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bevy_integration;
mod bridge;
mod commands;
mod config;
mod plugin;
mod resources;
mod tauri_setup;

use bevy::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tauri::{Manager, RunEvent};

use bevy_integration::create_bevy_app;
use plugin::handle_window_event;
use resources::PreBevyResources;
use tauri_setup::build_tauri_app;

fn main() {
    let pre_bevy = PreBevyResources::new().expect("Failed to initialize pre-Bevy resources");

    let mut tauri_app = build_tauri_app(&pre_bevy).expect("Failed to build Tauri application");

    let bevy_app: Rc<RefCell<Option<App>>> = Rc::new(RefCell::new(None));
    let pre_bevy = Rc::new(pre_bevy);

    loop {
        let bevy_app_clone = bevy_app.clone();
        let pre_bevy_clone = pre_bevy.clone();

        #[allow(deprecated)]
        tauri_app.run_iteration(move |app_handle, event| {
            handle_tauri_event(app_handle, event, &bevy_app_clone, &pre_bevy_clone);
        });

        if tauri_app.webview_windows().is_empty() {
            tauri_app.cleanup_before_exit();
            break;
        }

        if let Some(ref mut app) = *bevy_app.borrow_mut() {
            app.update();
        }
    }
}

fn handle_tauri_event(
    app_handle: &tauri::AppHandle,
    event: RunEvent,
    bevy_app: &Rc<RefCell<Option<App>>>,
    pre_bevy: &Rc<PreBevyResources>,
) {
    match event {
        RunEvent::Ready => {
            if bevy_app.borrow().is_none() {
                let window = app_handle
                    .get_webview_window("main")
                    .expect("Main window not found");

                let app = create_bevy_app(app_handle.clone(), window, pre_bevy);

                *bevy_app.borrow_mut() = Some(app);
            }
        }
        RunEvent::WindowEvent { event, .. } => {
            if let Some(ref mut app) = *bevy_app.borrow_mut() {
                handle_window_event(event, app);
            }
        }
        _ => {}
    }
}
