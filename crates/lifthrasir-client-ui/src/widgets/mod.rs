//! In-game HUD overlay (raw `bevy_ui`).
//!
//! A full-screen, pickable-ignored root (so clicks reach the world) hosting the
//! status bar, the chat box and the other HUD widgets. Built on
//! `OnEnter(GameState::InGame)` and torn down by `DespawnOnExit`; the widget
//! sub-plugins drive their marked elements.

use bevy::prelude::*;
use lifthrasir_client::core::state::GameState;

pub mod announcement;
pub mod card_compounding;
pub mod character_window;
pub mod chat_box;
pub mod cutin;
pub mod death_dialog;
pub mod emote;
pub mod escape_menu;
pub mod guild_window;
pub mod hotbar;
pub mod hud_layout;
pub mod info_modal;
pub mod inventory_window;
pub mod minimap;
pub mod mount;
pub mod navigation;
pub mod npc_dialog;
pub mod party;
pub mod player_context_menu;
pub mod pushcart_window;
pub mod settings_window;
pub mod shop_window;
pub mod skill_menu;
pub mod skill_target_toast;
pub mod skill_text_input;
pub mod skills_window;
pub mod status_bar;
pub mod status_icons;
pub mod storage_window;
pub mod system_dialog;
pub mod toasts;
pub mod trade_window;
pub mod vending_window;

pub struct InGameHudPlugin;

impl Plugin for InGameHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), show_hud);
        app.add_plugins((
            announcement::AnnouncementPlugin,
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
            status_bar::StatusBarPlugin,
            status_icons::StatusIconsPlugin,
            storage_window::StorageWindowPlugin,
        ));
        app.add_plugins(inventory_window::InventoryWindowPlugin);
        app.add_plugins(hud_layout::HudLayoutPlugin);
        app.add_plugins(skills_window::SkillsWindowPlugin);
        app.add_plugins(trade_window::TradeWindowPlugin);
        app.add_plugins(vending_window::VendingWindowPlugin);
        app.add_plugins(card_compounding::CardCompoundingPlugin);
        app.add_plugins(skill_menu::SkillMenuPlugin);
        app.add_plugins(skill_text_input::SkillTextInputPlugin);
        app.add_plugins(navigation::NavigationUiPlugin);
        app.add_plugins(skill_target_toast::SkillTargetToastPlugin);
        app.add_plugins(cutin::CutinPlugin);
        app.add_plugins(toasts::ToastsPlugin);
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
    status_bar::spawn_status_bar(&mut commands, root);
    character_window::shell::build(&mut commands, root);
    chat_box::spawn_chat_box(&mut commands, root);
    emote::spawn_emote_picker(&mut commands, root);
    guild_window::spawn_guild_window(&mut commands, root);
    hotbar::spawn_hotbar(&mut commands, root);
    inventory_window::scene::build(&mut commands, root);
    minimap::spawn_minimap(&mut commands, root, &asset_server);
    navigation::panel::spawn_navigation_panel(
        &mut commands,
        root,
        asset_server.load(lifthrasir_ui::theme::FONT_BODY),
    );
    party::spawn_party_window(&mut commands, root);
    pushcart_window::spawn_pushcart_window(&mut commands, root);
    skills_window::shell::build(&mut commands, root);
    storage_window::scene::build(&mut commands, root);
    toasts::spawn_toast_stack(&mut commands, root);
    trade_window::scene::build(&mut commands, root);
    vending_window::spawn_vending_window(&mut commands, root);
}
