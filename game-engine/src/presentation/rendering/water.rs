use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use bevy_auto_plugin::prelude::*;
use ro_formats::RswWater;

use crate::{
    domain::{
        system_sets::WaterRenderingSystems,
        world::{
            components::MapLoader, map::MapData, map_loader::MapRequestLoader,
            map_scoped::MapScoped,
        },
    },
    infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset},
    utils::constants::CELL_SIZE,
};

#[derive(Component)]
pub struct WaterSurface {
    pub water_level: f32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub animation_speed: f32,
    pub mesh_handle: Handle<Mesh>,
    pub material_handle: Handle<WaterMaterial>,
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
    pub tile_coords: Vec4,
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "ro://shaders/water.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "ro://shaders/water.wgsl".into()
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

/// Type alias for maps ready for water loading
type MapsReadyForWater<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MapLoader,
        &'static MapRequestLoader,
        &'static MapData,
    ),
    (
        Without<WaterSurface>,
        Without<WaterLoadingState>,
        With<MapData>,
    ),
>;

/// Temporary component to track water texture loading state
#[derive(Component)]
pub struct WaterLoadingState {
    pub(crate) texture_handle: Handle<Image>,
    pub(crate) water_tiles: Vec<(usize, usize)>,
    pub(crate) wave_height: f32,
    pub(crate) water_level: f32,
    pub(crate) wave_height_param: f32,
    pub(crate) wave_speed: f32,
    pub(crate) wave_pitch: f32,
    pub(crate) animation_speed: f32,
}

#[auto_add_system(
    plugin = crate::presentation::rendering::map_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = WaterRenderingSystems::WaterLoading)
)]
pub fn load_water_system(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    asset_server: Res<AssetServer>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: MapsReadyForWater,
) {
    for (entity, map_loader, _map_request, _) in query.iter() {
        let Some(world_handle) = map_loader.world.as_ref() else {
            continue;
        };

        let Some(world_asset) = world_assets.get(world_handle) else {
            continue;
        };

        let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let ground = &ground_asset.ground;
        let width = ground.width as usize;

        begin_water_loading(
            &mut commands,
            &asset_server,
            entity,
            &world_asset.world.water,
            (width, ground.height as usize),
            |x, y| {
                let heights = ground.surfaces[y * width + x].height;
                heights[0].max(heights[1]).max(heights[2]).max(heights[3])
            },
        );
    }
}

/// Queues the water surface of one map: picks the submerged tiles, starts the
/// texture load and leaves a [`WaterLoadingState`] behind for
/// [`finalize_water_loading_system`].
///
/// Tiles are ground cells addressed `(x, y)` over `grid`, `CELL_SIZE` wide, and
/// `tile_top` gives the highest of a tile's four corners. RO heights grow
/// downwards, so a corner above the wave plane is under water -- including
/// deep cells with no ground tile (`tile_up == -1`), which the terrain mesh
/// skips.
fn begin_water_loading(
    commands: &mut Commands,
    asset_server: &AssetServer,
    entity: Entity,
    water: &RswWater,
    grid: (usize, usize),
    tile_top: impl Fn(usize, usize) -> f32,
) {
    if water.level == 0.0 {
        return;
    }

    debug!(
        "Creating water surface at level: {}, wave_height: {}, wave_speed: {}, wave_pitch: {}, anim_speed: {}",
        water.level, water.wave_height, water.wave_speed, water.wave_pitch, water.anim_speed
    );

    // Per-tile water detection logic (based on GRF Editor)
    let wave_height = water.level - water.wave_height;
    let (width, height) = grid;
    let water_tiles: Vec<(usize, usize)> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| tile_top(x, y) > wave_height)
        .collect();

    if water_tiles.is_empty() {
        debug!("No water tiles detected for this map");
        return;
    }

    debug!("Detected {} water tiles, creating mesh", water_tiles.len());

    commands.entity(entity).insert(WaterLoadingState {
        texture_handle: load_water_texture(water.water_type, 0, asset_server),
        water_tiles,
        wave_height,
        water_level: water.level,
        wave_height_param: water.wave_height,
        wave_speed: water.wave_speed,
        wave_pitch: water.wave_pitch,
        animation_speed: water.anim_speed as f32,
    });
}

/// Queues the water surface of a map served by a converted glb, which carries
/// no GND surfaces: the tile heights come from the GAT instead.
///
/// The GAT grid is twice the resolution the water mesh is tiled at, so a tile
/// takes the highest corner of the four GAT cells covering it. That is a
/// superset of the GND corners the native path sees -- it can only flood a
/// tile the terrain already dips below the wave plane in.
#[cfg(feature = "map-gltf")]
pub fn begin_gltf_map_water(
    commands: &mut Commands,
    asset_server: &AssetServer,
    entity: Entity,
    water: &RswWater,
    altitude: &ro_formats::RoAltitude,
) {
    let grid = (altitude.width as usize / 2, altitude.height as usize / 2);

    begin_water_loading(commands, asset_server, entity, water, grid, |x, y| {
        [(0, 0), (1, 0), (0, 1), (1, 1)]
            .into_iter()
            .filter_map(|(dx, dy)| altitude.get_cell(x * 2 + dx, y * 2 + dy))
            .flat_map(|cell| cell.height)
            .fold(f32::MIN, f32::max)
    });
}

// Maximum water parameter values to prevent excessive movement in some maps
const MAX_WAVE_HEIGHT: f32 = 10.0;
const MAX_WAVE_SPEED: f32 = 10.0;
const MAX_WAVE_PITCH: f32 = 100.0;
const MIN_WAVE_PITCH: f32 = 0.1;

// Water mesh subdivision (8x8 = 128 triangles per tile)
const WATER_TILE_SUBDIVISIONS: usize = 8;

/// System to finalize water loading once textures are ready
#[auto_add_system(
    plugin = crate::presentation::rendering::map_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = WaterRenderingSystems::WaterFinalization)
)]
pub fn finalize_water_loading_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &WaterLoadingState)>,
) {
    for (entity, loading_state) in query.iter() {
        // Check if texture is loaded
        if asset_server.is_loaded_with_dependencies(&loading_state.texture_handle) {
            debug!("Water texture loaded, creating mesh and material");

            // Fix sampler for tiling
            match images.get_mut(&loading_state.texture_handle) {
                Some(mut image) => {
                    // Override sampler to Repeat mode for proper tiling across water surface
                    // Bevy's built-in JPEG loader uses Default (ClampToEdge), but we need Repeat
                    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        address_mode_w: ImageAddressMode::Repeat,
                        mag_filter: ImageFilterMode::Linear,
                        min_filter: ImageFilterMode::Linear,
                        mipmap_filter: ImageFilterMode::Linear,
                        ..Default::default()
                    });
                }
                _ => {
                    warn!("Water texture handle loaded but image data not in Assets<Image>");
                }
            }

            // Generate procedural normal map for water
            let normal_map = generate_water_normal_map(&mut images);

            // Create single mesh containing all water tiles
            let water_mesh =
                create_water_tiles_mesh(&loading_state.water_tiles, loading_state.wave_height);
            let mesh_handle = meshes.add(water_mesh);

            // Calculate texture scale based on map size
            let texture_scale = 0.125;

            // Pre-scale wave_height for Gerstner wave formula (a = amplitude / k)
            // This makes the final amplitude match the wave_height parameter
            let wave_pitch = loading_state
                .wave_pitch
                .clamp(MIN_WAVE_PITCH, MAX_WAVE_PITCH);
            let k = 2.0 * std::f32::consts::PI / wave_pitch;
            let scaled_wave_height =
                (loading_state.wave_height_param.min(MAX_WAVE_HEIGHT) * k).min(MAX_WAVE_HEIGHT);

            // Create a single material for all water with loaded texture
            let water_extension = WaterExtension {
                water_data: WaterData {
                    wave_params: Vec4::new(
                        scaled_wave_height,
                        loading_state.wave_speed.min(MAX_WAVE_SPEED),
                        wave_pitch,
                        0.0,
                    ),
                    animation_params: Vec4::ZERO,
                    tile_coords: Vec4::new(0.0, 0.0, texture_scale, 0.0),
                },
                water_texture: loading_state.texture_handle.clone(),
                normal_map,
            };

            let water_material = WaterMaterial {
                base: StandardMaterial {
                    base_color: Color::srgba(1.0, 1.0, 1.0, 0.05),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.1,
                    metallic: 0.0,
                    reflectance: 0.9,
                    ..default()
                },
                extension: water_extension,
            };

            let material_handle = materials.add(water_material);

            // Spawn single entity with all water
            commands.spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(material_handle.clone()),
                Transform::IDENTITY,
                MapScoped,
            ));

            // Add water surface component to the main entity and remove loading state
            commands
                .entity(entity)
                .insert((
                    WaterSurface {
                        water_level: loading_state.water_level,
                        wave_height: loading_state.wave_height_param,
                        wave_speed: loading_state.wave_speed,
                        wave_pitch: loading_state.wave_pitch,
                        animation_speed: loading_state.animation_speed,
                        mesh_handle,
                        material_handle,
                    },
                    WaterAnimation {
                        time: 0.0,
                        uv_offset: Vec2::ZERO,
                    },
                ))
                .remove::<WaterLoadingState>();

            debug!("Water rendering setup complete");
        }
    }
}

#[auto_add_system(
    plugin = crate::presentation::rendering::map_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = WaterRenderingSystems::WaterAnimation)
)]
pub fn animate_water_system(
    time: Res<Time>,
    mut water_query: Query<(&WaterSurface, &mut WaterAnimation)>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    for (water_surface, mut water_animation) in water_query.iter_mut() {
        water_animation.time += time.delta_secs();

        let uv_scroll_speed = water_surface.animation_speed * 0.01;
        water_animation.uv_offset +=
            Vec2::new(uv_scroll_speed, uv_scroll_speed * 0.7) * time.delta_secs();

        if let Some(mut material) = materials.get_mut(&water_surface.material_handle) {
            material.extension.water_data.wave_params.w = water_animation.time;
            material.extension.water_data.animation_params = Vec4::new(
                water_animation.uv_offset.x,
                water_animation.uv_offset.y,
                0.0,
                0.0,
            );
        }
    }
}

/// Create a single mesh containing all water tiles
fn create_water_tiles_mesh(water_tiles: &[(usize, usize)], water_y: f32) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut tangents = Vec::new();
    let mut indices = Vec::new();

    let subdivisions = WATER_TILE_SUBDIVISIONS;
    let verts_per_side = subdivisions + 1;
    let step_size = CELL_SIZE / subdivisions as f32;

    // Create subdivided mesh for each tile
    for &(tile_x, tile_y) in water_tiles.iter() {
        let base_vertex = positions.len() as u32;

        let tile_world_x = tile_x as f32 * CELL_SIZE;
        let tile_world_z = tile_y as f32 * CELL_SIZE;

        // Create vertex grid for this tile (5x5 for 4x4 subdivision)
        for row in 0..verts_per_side {
            for col in 0..verts_per_side {
                let x = tile_world_x + col as f32 * step_size;
                let z = tile_world_z + row as f32 * step_size;

                positions.push([x, water_y, z]);
                normals.push([0.0, -1.0, 0.0]);

                // UV coordinates (0-1 within tile, shader calculates world UVs)
                let u = col as f32 / subdivisions as f32;
                let v = row as f32 / subdivisions as f32;
                uvs.push([u, v]);

                // Tangent vector (X-axis aligned, w=1.0 for handedness)
                tangents.push([1.0, 0.0, 0.0, 1.0]);
            }
        }

        // Create indices for quads
        for row in 0..subdivisions {
            for col in 0..subdivisions {
                let i0 = base_vertex + (row * verts_per_side + col) as u32;
                let i1 = i0 + 1;
                let i2 = i0 + verts_per_side as u32;
                let i3 = i2 + 1;

                // Triangle 1: bottom-left, bottom-right, top-right
                indices.push(i0);
                indices.push(i1);
                indices.push(i3);

                // Triangle 2: bottom-left, top-right, top-left
                indices.push(i0);
                indices.push(i3);
                indices.push(i2);
            }
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Load water texture from GRF based on water type and frame
fn load_water_texture(water_type: u32, frame: u32, asset_server: &AssetServer) -> Handle<Image> {
    let texture_path = format!("ro://data\\texture\\워터\\water{water_type}{frame:02}.jpg");

    // Use AssetServer to load the texture directly
    asset_server.load(texture_path)
}

/// Generate a procedural normal map for water surface detail
fn generate_water_normal_map(images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    const SIZE: u32 = 512;
    let mut data = vec![0u8; (SIZE * SIZE * 4) as usize];

    // Generate multiple octaves of noise for realistic water surface
    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = ((y * SIZE + x) * 4) as usize;

            // Create multiple waves at different frequencies
            let fx = x as f32 / SIZE as f32;
            let fy = y as f32 / SIZE as f32;

            // Calculate normal from height gradient
            // Sample neighboring pixels for gradient calculation
            let dx = if x > 0 && x < SIZE - 1 {
                let fx_prev = (x - 1) as f32 / SIZE as f32;
                let fx_next = (x + 1) as f32 / SIZE as f32;

                let h_prev = (fx_prev * 8.0 * std::f32::consts::PI).sin() * 0.5
                    + (fx_prev * 16.0 * std::f32::consts::PI + 1.57).sin() * 0.25
                    + ((fx_prev * 32.0 + fy * 24.0) * std::f32::consts::PI).sin() * 0.125;

                let h_next = (fx_next * 8.0 * std::f32::consts::PI).sin() * 0.5
                    + (fx_next * 16.0 * std::f32::consts::PI + 1.57).sin() * 0.25
                    + ((fx_next * 32.0 + fy * 24.0) * std::f32::consts::PI).sin() * 0.125;

                (h_next - h_prev) * 0.5
            } else {
                0.0
            };

            let dy = if y > 0 && y < SIZE - 1 {
                let fy_prev = (y - 1) as f32 / SIZE as f32;
                let fy_next = (y + 1) as f32 / SIZE as f32;

                let h_prev = (fy_prev * 8.0 * std::f32::consts::PI).cos() * 0.5
                    + (fy_prev * 16.0 * std::f32::consts::PI + 0.78).cos() * 0.25
                    + ((fx * 32.0 + fy_prev * 24.0) * std::f32::consts::PI).sin() * 0.125;

                let h_next = (fy_next * 8.0 * std::f32::consts::PI).cos() * 0.5
                    + (fy_next * 16.0 * std::f32::consts::PI + 0.78).cos() * 0.25
                    + ((fx * 32.0 + fy_next * 24.0) * std::f32::consts::PI).sin() * 0.125;

                (h_next - h_prev) * 0.5
            } else {
                0.0
            };

            // Convert gradient to normal (tangent space)
            let normal = Vec3::new(-dx * 2.0, -dy * 2.0, 1.0).normalize();

            // Encode normal to RGB (0-1 range)
            data[idx] = ((normal.x * 0.5 + 0.5) * 255.0) as u8; // R
            data[idx + 1] = ((normal.y * 0.5 + 0.5) * 255.0) as u8; // G
            data[idx + 2] = ((normal.z * 0.5 + 0.5) * 255.0) as u8; // B
            data[idx + 3] = 255; // A
        }
    }

    let normal_image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );

    images.add(normal_image)
}
