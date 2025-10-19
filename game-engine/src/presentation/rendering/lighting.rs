use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use std::f32::consts::PI;

const MAX_LUX: f32 = 10_000.0; // Bright daylight

use crate::{
    domain::world::components::MapLoader,
    infrastructure::assets::loaders::RoWorldAsset,
    infrastructure::ro_formats::{RswLight, RswLightObj, RswObject},
};

/// Enhanced Lighting Plugin that creates realistic lighting from RSW data
pub struct EnhancedLightingPlugin;

impl Plugin for EnhancedLightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (setup_enhanced_map_lighting, cleanup_map_lights));
    }
}

#[derive(Component)]
pub struct MapLight;

/// System to setup enhanced map lighting based on RSW data
pub fn setup_enhanced_map_lighting(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    query: Query<(Entity, &MapLoader), Without<MapLight>>,
) {
    for (entity, map_loader) in query.iter() {
        if let Some(world_handle) = &map_loader.world {
            if let Some(world_asset) = world_assets.get(world_handle) {
                let world = &world_asset.world;

                setup_directional_light(&mut commands, &world.light);
                setup_ambient_light(&mut commands, &world.light);
                spawn_enhanced_point_lights(&mut commands, &world.objects);

                commands.entity(entity).insert(MapLight);
            }
        }
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

    commands.spawn((
        DirectionalLight {
            illuminance,
            color: light_color,
            shadows_enabled: true,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 1.8,
            ..default()
        },
        Transform::from_translation(Vec3::ZERO).looking_to(light_direction, Vec3::NEG_Y),
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 200.0,
            maximum_distance: 5000.0,
            overlap_proportion: 0.2,
            ..default()
        }
        .build(),
        MapLight,
    ));
}

/// Setup enhanced ambient lighting from RSW ambient values
fn setup_ambient_light(commands: &mut Commands, rsw_light: &RswLight) {
    let ambient_color = Color::srgb(
        rsw_light.ambient[0],
        rsw_light.ambient[1],
        rsw_light.ambient[2],
    );

    commands.insert_resource(AmbientLight {
        color: ambient_color,
        brightness: 500.0, // Ensure minimum ambient light for softer shadows
        affects_lightmapped_meshes: false,
    });
}

/// Spawn enhanced point lights from RSW light objects
fn spawn_enhanced_point_lights(commands: &mut Commands, rsw_objects: &[RswObject]) {
    let mut point_light_count = 0;

    for obj in rsw_objects.iter() {
        if let RswObject::Light(light_obj) = obj {
            spawn_point_light(commands, light_obj);
            point_light_count += 1;
        }
    }

    debug!("Spawned {} point lights", point_light_count);
}

/// Spawn individual point light from RSW light object
fn spawn_point_light(commands: &mut Commands, light_obj: &RswLightObj) {
    let position = Vec3::new(
        light_obj.position[0],
        light_obj.position[1],
        light_obj.position[2],
    );

    let light_color = Color::srgb(light_obj.color[0], light_obj.color[1], light_obj.color[2]);

    let base_intensity = 1000.0;
    let color_brightness = (light_obj.color[0] + light_obj.color[1] + light_obj.color[2]) / 3.0;
    let final_intensity = base_intensity * color_brightness;

    let radius = 0.3;

    commands.spawn((
        PointLight {
            intensity: final_intensity,
            color: light_color,
            range: light_obj.range,
            radius,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(position),
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 200.0,
            maximum_distance: 5000.0,
            overlap_proportion: 0.2,
            ..default()
        }
        .build(),
        MapLight,
    ));
}

fn calculate_global_lux(light: &RswLight) -> f32 {
    let diffuse_intensity = light.diffuse.iter().fold(0.0f32, |acc, &x| acc.max(x));

    MAX_LUX * diffuse_intensity * light.opacity
}

/// System to cleanup map lights when switching maps
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
