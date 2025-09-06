use bevy::prelude::*;

#[derive(Component)]
pub struct MapData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub cell_size: f32,
}

#[derive(Component)]
pub struct TerrainChunk {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Component)]
pub struct WaterPlane {
    pub level: f32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub anim_speed: f32,
    pub time: f32,
}

#[derive(Component)]
pub struct LightEnvironment {
    pub longitude: f32,
    pub latitude: f32,
    pub diffuse: Color,
    pub ambient: Color,
    pub direction: Vec3,
}

impl LightEnvironment {
    pub fn from_angles(
        longitude: u32,
        latitude: u32,
        diffuse: [f32; 3],
        ambient: [f32; 3],
    ) -> Self {
        let lon_rad = (longitude as f32).to_radians();
        let lat_rad = (latitude as f32).to_radians();

        let direction = Vec3::new(
            lat_rad.cos() * lon_rad.sin(),
            lat_rad.sin(),
            lat_rad.cos() * lon_rad.cos(),
        )
        .normalize();

        Self {
            longitude: longitude as f32,
            latitude: latitude as f32,
            diffuse: Color::srgb(diffuse[0], diffuse[1], diffuse[2]),
            ambient: Color::srgb(ambient[0], ambient[1], ambient[2]),
            direction,
        }
    }
}

#[derive(Component)]
pub struct MapCamera {
    pub zoom: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub rotation: f32,
    pub target: Vec3,
}
