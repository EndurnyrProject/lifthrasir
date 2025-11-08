use bevy_auto_plugin::prelude::*;

/// Plugin for character domain logic.
///
/// System organization (6 chained groups with strict ordering):
/// 1. Character Server Systems: char_server_update_system, update_char_client
/// 2. Protocol Event Handlers: handle_character_server_connected, handle_character_created_protocol, etc.
/// 3. Domain Event Handlers: handle_request_character_list, handle_select_character, spawn_unified_character_from_selection, etc.
/// 4. Post-Selection Handlers: handle_zone_server_info, handle_character_created, handle_character_deleted, etc.
/// 5. Zone Server Systems: zone_server_update_system, time_sync_system, handle_zone_server_connected_protocol, etc.
/// 6. Map Loading Systems: start_map_loading_timer, detect_map_loading_timeout, detect_map_load_complete, etc.
///
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct CharacterDomainAutoPlugin;
