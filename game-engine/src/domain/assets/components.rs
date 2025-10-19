#![allow(dead_code)]

use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(Component)]
pub struct WaterSurface {
    pub water_level: f32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub animation_speed: f32,
    pub mesh_handle: Handle<Mesh>,
    pub material_handle: Handle<ExtendedMaterial<StandardMaterial, WaterExtension>>,
}

#[derive(Component)]
pub struct WaterAnimation {
    pub time: f32,
    pub uv_offset: Vec2,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterExtension {
    #[uniform(100)]
    pub water_data: WaterData,
    #[texture(101)]
    #[sampler(102)]
    pub water_texture: Handle<Image>,
    #[texture(103)]
    #[sampler(104)]
    pub normal_map: Handle<Image>,
}

#[derive(Debug, Clone, ShaderType)]
pub struct WaterData {
    pub wave_params: Vec4,
    pub animation_params: Vec4,
    pub tile_coords: Vec4, // xy = tile position, zw = texture scale
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

impl Default for WaterExtension {
    fn default() -> Self {
        Self {
            water_data: WaterData {
                wave_params: Vec4::new(0.2, 2.0, 50.0, 0.0),
                animation_params: Vec4::ZERO,
                tile_coords: Vec4::ZERO,
            },
            water_texture: Handle::default(),
            normal_map: Handle::default(),
        }
    }
}
