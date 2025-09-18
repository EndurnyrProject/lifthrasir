use crate::domain::character::CharacterData;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct CharacterListResource {
    pub characters: Vec<Option<CharacterData>>,
    pub max_slots: u8,
    pub available_slots: u8,
    pub premium_slots: u8,
}

#[derive(Resource, Default)]
pub struct CharacterSelectionResource {
    pub selected_slot: Option<u8>,
    pub hovering_slot: Option<u8>,
    pub selected_character: Option<CharacterData>,
}

#[derive(Resource)]
pub struct CharacterSelectionAssets {
    pub no_char_frame: Handle<Image>,
    pub with_char_frame: Handle<Image>,
}

impl CharacterSelectionAssets {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            no_char_frame: asset_server.load("ro://textures/ui/no_char_frame.png"),
            with_char_frame: asset_server.load("ro://textures/ui/frame_with_char.png"),
        }
    }
}
