//! In-game HUD overlay (raw `bevy_ui`).
//!
//! A full-screen, pickable-ignored root (so clicks reach the world) hosting the
//! status-frame and chat-box widgets. Built on `OnEnter(GameState::InGame)` and torn
//! down by `DespawnOnExit`; the two widget sub-plugins drive their marked elements.

use bevy::prelude::*;
use game_engine::core::state::GameState;

pub mod announcement;
pub mod character_info;
pub mod character_window;
pub mod chat_box;
pub mod chrome;
pub mod death_dialog;
pub mod draggable;
pub mod emote;
pub mod guild_window;
pub mod hotbar;
pub mod minimap;
pub mod npc_dialog;
pub mod party;
pub mod placeholder;
pub mod pushcart_window;
pub mod settings_window;
pub mod shop_window;
pub mod status_icons;
pub mod storage_window;
pub mod system_dialog;

pub struct InGameHudPlugin;

impl Plugin for InGameHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), show_hud);
        app.add_plugins((
            announcement::AnnouncementPlugin,
            character_info::CharacterInfoPlugin,
            character_window::CharacterWindowPlugin,
            chat_box::ChatBoxPlugin,
            emote::EmotePickerPlugin,
            guild_window::GuildWindowPlugin,
            hotbar::HotbarWidgetPlugin,
            minimap::MinimapPlugin,
            npc_dialog::NpcDialogPlugin,
            party::PartyPlugin,
            pushcart_window::PushcartWindowPlugin,
            settings_window::SettingsWindowPlugin,
            shop_window::ShopWindowPlugin,
            status_icons::StatusIconsPlugin,
            storage_window::StorageWindowPlugin,
        ));
    }
}

fn show_hud(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(GameState::InGame),
        ))
        .id();

    announcement::spawn_announcement_layer(&mut commands, root);
    character_info::spawn_status_frame(&mut commands, root, &asset_server);
    character_window::shell::build(&mut commands, root);
    chat_box::spawn_chat_box(&mut commands, root, &asset_server);
    emote::spawn_emote_picker(&mut commands, root);
    guild_window::scene::build(&mut commands, root);
    hotbar::spawn_hotbar(&mut commands, root, &asset_server);
    minimap::spawn_minimap(&mut commands, root, &asset_server);
    party::spawn_party_window(&mut commands, root);
    pushcart_window::spawn_pushcart_window(&mut commands, root);
    storage_window::scene::build(&mut commands, root);
    status_icons::spawn_status_bar(&mut commands, root);
}
