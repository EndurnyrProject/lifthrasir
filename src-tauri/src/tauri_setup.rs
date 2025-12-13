use crate::commands;
use crate::resources::PreBevyResources;

pub fn build_tauri_app(pre_bevy: &PreBevyResources) -> Result<tauri::App, tauri::Error> {
    tauri::Builder::default()
        .manage(pre_bevy.app_bridge.clone())
        .manage(pre_bevy.composite_source.clone())
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
        ])
        .build(tauri::generate_context!())
}
