use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use std::f32::consts::PI;

const MAX_LUX: f32 = 3_000.0; // Sun illuminance; kept low so point lights read against it (camera Exposure compensates overall brightness)

use crate::{
    domain::system_sets::MiscRenderingSystems,
    domain::world::components::MapLoader,
    domain::world::map_scoped::MapScoped,
    infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset},
    infrastructure::ro_formats::{RswLight, RswLightObj, RswObject},
    utils::{get_map_dimensions_from_ground, rsw_position_to_bevy},
};

/// Enhanced Lighting Plugin that creates realistic lighting from RSW data
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct EnhancedLightingPlugin;

#[derive(Component)]
pub struct MapLight;

/// System to setup enhanced map lighting based on RSW data
#[auto_add_system(
    plugin = crate::presentation::rendering::lighting::EnhancedLightingPlugin,
    schedule = Update,
    config(in_set = MiscRenderingSystems::LightingSetup)
)]
pub fn setup_enhanced_map_lighting(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: Query<(Entity, &MapLoader), Without<MapLight>>,
) {
    for (entity, map_loader) in query.iter() {
        let Some(world_handle) = &map_loader.world else {
            continue;
        };
        let Some(world_asset) = world_assets.get(world_handle) else {
            continue;
        };
        let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let world = &world_asset.world;
        let (map_width, map_height) = get_map_dimensions_from_ground(&ground_asset.ground);

        setup_directional_light(&mut commands, &world.light);
        setup_ambient_light(&mut commands, &world.light);
        spawn_enhanced_point_lights(&mut commands, &world.objects, map_width, map_height);

        commands.entity(entity).insert(MapLight);
    }
}

/// Setup directional light (sun/moon) from RSW global lighting
fn setup_directional_light(commands: &mut Commands, rsw_light: &RswLight) {
    // RSW coordinates: longitude 0-360°, latitude 0-180°
    // Convert to Bevy's coordinate system with proper directional light angles
    let longitude_deg = rsw_light.longitude as f32;
    let latitude_deg = rsw_light.latitude as f32;

    // Convert RSW angles to Bevy world space
    // RSW latitude 0° = zenith (straight down), 90° = horizon, 180° = nadir (straight up)
    // Bevy Y-up: we want latitude 45° to be a nice diagonal shadow
    let elevation_deg = 90.0 - latitude_deg;
    let azimuth_deg = longitude_deg;

    let elevation_rad = elevation_deg * PI / 180.0;
    let azimuth_rad = azimuth_deg * PI / 180.0;

    let light_color = Color::srgb(
        rsw_light.diffuse[0],
        rsw_light.diffuse[1],
        rsw_light.diffuse[2],
    );

    let sun_dir_x = azimuth_rad.cos() * elevation_rad.cos();
    let sun_dir_y = elevation_rad.sin();
    let sun_dir_z = azimuth_rad.sin() * elevation_rad.cos();
    let light_direction = Vec3::new(sun_dir_x, sun_dir_y, sun_dir_z).normalize();
    let illuminance = calculate_global_lux(rsw_light);

    // Bevy 0.17 uses basic orthographic culling for cascades. Per-cascade
    // frustum culling (github.com/bevyengine/bevy/issues/10397) is not yet implemented.
    // The distance reduction compensates for this limitation.
    commands.spawn((
        DirectionalLight {
            illuminance,
            color: light_color,
            shadow_maps_enabled: true,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 1.8,
            ..default()
        },
        Transform::from_translation(Vec3::ZERO).looking_to(light_direction, Vec3::NEG_Y),
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: 200.0,
            maximum_distance: 1500.0,
            overlap_proportion: 0.2,
            ..default()
        }
        .build(),
        MapLight,
        MapScoped,
    ));
}

/// Setup enhanced ambient lighting from RSW ambient values
fn setup_ambient_light(commands: &mut Commands, rsw_light: &RswLight) {
    let ambient_color = Color::srgb(
        rsw_light.ambient[0],
        rsw_light.ambient[1],
        rsw_light.ambient[2],
    );

    commands.insert_resource(GlobalAmbientLight {
        color: ambient_color,
        brightness: 100.0, // Low fill so the sun and point lights keep contrast (Exposure restores overall brightness)
        affects_lightmapped_meshes: false,
    });
}

/// Spawn enhanced point lights from RSW light objects
fn spawn_enhanced_point_lights(
    commands: &mut Commands,
    rsw_objects: &[RswObject],
    map_width: f32,
    map_height: f32,
) {
    let mut point_light_count = 0;

    for obj in rsw_objects.iter() {
        if let RswObject::Light(light_obj) = obj {
            spawn_point_light(commands, light_obj, map_width, map_height);
            point_light_count += 1;
        }
    }

    debug!("Spawned {} point lights", point_light_count);
}

/// Spawn individual point light from RSW light object
fn spawn_point_light(
    commands: &mut Commands,
    light_obj: &RswLightObj,
    map_width: f32,
    map_height: f32,
) {
    let position = rsw_position_to_bevy(light_obj.position, map_width, map_height);

    let light_color = Color::srgb(light_obj.color[0], light_obj.color[1], light_obj.color[2]);

    // Tuned against the lowered sun/ambient baseline and the camera Exposure/Bloom.
    // The RSW color already scales emitted radiance, so dim torches stay dim without a
    // separate brightness multiplier.
    let intensity = 3_000_000.0;

    let radius = 0.3;

    commands.spawn((
        PointLight {
            intensity,
            color: light_color,
            range: light_obj.range,
            radius,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(position),
        MapLight,
        MapScoped,
    ));
}

fn calculate_global_lux(light: &RswLight) -> f32 {
    let diffuse_intensity = light.diffuse.iter().fold(0.0f32, |acc, &x| acc.max(x));

    MAX_LUX * diffuse_intensity * light.opacity
}

/// System to cleanup map lights when switching maps
#[auto_add_system(
    plugin = crate::presentation::rendering::lighting::EnhancedLightingPlugin,
    schedule = Update,
    config(in_set = MiscRenderingSystems::LightingCleanup)
)]
pub fn cleanup_map_lights(
    _commands: Commands,
    _query: Query<Entity, With<MapLight>>,
    // This system will be expanded later to detect map changes
    // For now, it provides the foundation for cleanup
) {
    // TODO: Add logic to detect when a new map is being loaded
    // and cleanup old lighting entities
    // This will be triggered by map change events in future phases
}
