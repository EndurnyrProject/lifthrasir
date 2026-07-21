use crate::core::state::GameState;
use crate::domain::entities::character::components::{
    CharacterData, CharacterMeta, status::CharacterStatus,
};
use crate::domain::entities::character::{
    SpawnCharacterSpriteEvent, add_gameplay_components_to_entity,
};
use crate::domain::entities::components::{EntityName, NetworkEntity};
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::types::ObjectType;
use crate::domain::settings::Settings;
use crate::domain::world::components::MapLoader;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::assets::loaders::RoGroundAsset;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::{SpatialAudioEmitter, SpatialAudioReceiver};
use bevy_persistent::prelude::Persistent;
use net_contract::state::UserSession;

/// Completes the selected character entity when the first map has loaded.
#[allow(clippy::too_many_arguments)]
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = OnEnter(GameState::InGame)
)]
pub fn spawn_character_sprite_on_game_start(
    mut commands: Commands,
    mut spawn_events: MessageWriter<SpawnCharacterSpriteEvent>,
    spawn_context: Res<MapSpawnContext>,
    mut entity_registry: ResMut<EntityRegistry>,
    user_session: Res<UserSession>,
    characters: Query<(Entity, &CharacterMeta, &CharacterData)>,
    map_loaders: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    existing_player: Query<(), With<LocalPlayer>>,
    settings: Res<Persistent<Settings>>,
) {
    // Warps preserve the local player; the warp path only needs repositioning.
    if !existing_player.is_empty() {
        return;
    }

    let Some((character_entity, _, character)) = characters
        .iter()
        .find(|(_, meta, _)| meta.char_id == spawn_context.character_id)
    else {
        error!(
            "Character entity not found for char_id: {}",
            spawn_context.character_id
        );
        return;
    };

    add_gameplay_components_to_entity(&mut commands.entity(character_entity));

    let account_id = user_session.tokens.account_id;
    let char_id = spawn_context.character_id;
    commands.entity(character_entity).insert((
        NetworkEntity::new(account_id, char_id, ObjectType::Pc),
        LocalPlayer,
        CharacterStatus::default(),
        EntityName::new(character.name.clone()),
        SpatialAudioReceiver,
        SpatialAudioEmitter::default(),
        settings.keybinds.to_input_map(),
    ));
    entity_registry.set_local_player(character_entity, char_id);

    let map_loader = map_loaders
        .single()
        .expect("MapLoader must exist before entering the game");
    let ground = ground_assets
        .get(&map_loader.ground)
        .expect("ground asset must be loaded before entering the game");
    let world_position = spawn_coords_to_world_position(
        spawn_context.spawn_x,
        spawn_context.spawn_y,
        ground.ground.width,
        ground.ground.height,
    );

    commands.entity(character_entity).insert((
        Transform::from_translation(world_position),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
    spawn_events.write(SpawnCharacterSpriteEvent {
        character_entity,
        spawn_position: world_position,
    });

    info!(
        "Spawned local player {:?} (char_id: {}, account_id: {}) '{}'",
        character_entity, char_id, account_id, character.name
    );
}
