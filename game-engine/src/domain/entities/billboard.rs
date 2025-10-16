use bevy::{
    prelude::*,
    render::{mesh::Indices, render_asset::RenderAssetUsages, render_resource::PrimitiveTopology},
};

/// Marker component for entities that should always face the camera
#[derive(Component, Debug, Clone, Copy)]
pub struct Billboard;

/// Component linking a 3D sprite to its character entity
#[derive(Component, Debug, Clone)]
pub struct Character3dSprite {
    pub character_entity: Entity,
    pub sprite_size: Vec2,
}

/// Resource containing the shared quad mesh used by all character billboards
#[derive(Resource, Debug, Clone)]
pub struct SharedSpriteQuad {
    pub mesh: Handle<Mesh>,
}

/// Creates a centered quad mesh for billboard sprites
/// Vertices range from -0.5 to 0.5 on both axes, centered at origin
/// UV coordinates are flipped vertically (V from 1.0 to 0.0) for correct sprite orientation
pub fn create_sprite_quad_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    // Vertices: Centered quad from -0.5 to 0.5
    // Order: Bottom-left, Bottom-right, Top-right, Top-left
    let positions: Vec<[f32; 3]> = vec![
        [-0.5, -0.5, 0.0], // Bottom-left
        [0.5, -0.5, 0.0],  // Bottom-right
        [0.5, 0.5, 0.0],   // Top-right
        [-0.5, 0.5, 0.0],  // Top-left
    ];

    // UV coordinates: Flipped V axis for correct sprite orientation
    let uvs: Vec<[f32; 2]> = vec![
        [0.0, 1.0], // Bottom-left -> texture bottom
        [1.0, 1.0], // Bottom-right -> texture bottom
        [1.0, 0.0], // Top-right -> texture top
        [0.0, 0.0], // Top-left -> texture top
    ];

    // Normals: All pointing forward (towards camera in billboard space)
    let normals: Vec<[f32; 3]> = vec![[0.0, 0.0, 1.0]; 4];

    // Indices: Two triangles forming a quad
    let indices = vec![
        0, 1, 2, // First triangle (bottom-left, bottom-right, top-right)
        2, 3, 0, // Second triangle (top-right, top-left, bottom-left)
    ];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// System to create the shared sprite quad mesh resource at startup
pub fn setup_shared_sprite_quad(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let quad_mesh = create_sprite_quad_mesh();
    let mesh_handle = meshes.add(quad_mesh);

    commands.insert_resource(SharedSpriteQuad { mesh: mesh_handle });

    info!("Initialized shared sprite quad mesh for 3D billboards");
}

/// System that makes billboard entities always face the camera
/// Copies the camera's rotation directly to each billboard transform
/// Runs after TransformPropagate to ensure proper ordering
pub fn billboard_rotation_system(
    camera_query: Query<&Transform, With<Camera3d>>,
    mut billboard_query: Query<&mut Transform, (With<Billboard>, Without<Camera3d>)>,
) {
    // Get the camera transform
    let Ok(camera_transform) = camera_query.single() else {
        return; // No camera or multiple cameras, skip this frame
    };

    // Copy camera rotation to all billboards
    for mut billboard_transform in billboard_query.iter_mut() {
        billboard_transform.rotation = camera_transform.rotation;
    }
}

/// Plugin that registers billboard systems and resources
pub struct BillboardPlugin;

impl Plugin for BillboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_shared_sprite_quad)
            .add_systems(
                PostUpdate,
                billboard_rotation_system
                    .after(bevy::transform::TransformSystem::TransformPropagate),
            );
    }
}
