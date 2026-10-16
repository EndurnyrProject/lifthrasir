use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use bevy_auto_plugin::prelude::*;

/// Marker component for entities that should always face the camera
#[derive(Component, Debug, Clone, Copy)]
pub struct Billboard;

/// Marker for billboards rendered on the equipment-window preview layer.
/// These face the preview camera instead of the primary follow camera.
/// Added by the equipment preview spawn path (equipment window preview).
#[derive(Component, Debug, Clone, Copy)]
pub struct PreviewBillboard;

/// Marker for the equipment-window preview camera. The preview billboard
/// facing system orients `PreviewBillboard` entities at this camera.
/// Added by the equipment preview camera spawn path (equipment window preview).
#[derive(Component, Debug, Clone, Copy)]
pub struct EquipmentPreviewCamera;

/// Query filter: the active 3D camera (gameplay follow or selection-screen
/// preview), excluding the equipment-window preview camera and billboards.
type ActiveCameraFilter = (
    With<Camera3d>,
    Without<EquipmentPreviewCamera>,
    Without<Billboard>,
);

/// Query filter: world billboards faced by the active camera, excluding the
/// equipment-window preview-layer billboards.
type WorldBillboardFilter = (
    With<Billboard>,
    Without<PreviewBillboard>,
    Without<Camera3d>,
);

/// Query filter: equipment-window preview-layer billboards.
type PreviewBillboardFilter = (With<Billboard>, With<PreviewBillboard>, Without<Camera3d>);

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
#[auto_add_system(
    plugin = crate::rendering::billboard::BillboardPlugin,
    schedule = Startup
)]
fn setup_shared_sprite_quad(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let quad_mesh = create_sprite_quad_mesh();
    let mesh_handle = meshes.add(quad_mesh);

    commands.insert_resource(SharedSpriteQuad { mesh: mesh_handle });

    debug!("Initialized shared sprite quad mesh for 3D billboards");
}

/// System that makes world billboard entities always face the active camera.
/// Copies that camera's rotation directly to each billboard transform — the
/// gameplay follow camera in-game, or the orthographic preview camera on the
/// character-selection / character-creation screens.
/// The equipment-window preview camera is excluded; its `PreviewBillboard`
/// layer is faced separately by `preview_billboard_rotation_system`. Only one
/// such camera exists per screen, so `single()` is unambiguous.
/// Runs after TransformPropagate to ensure proper ordering
#[auto_add_system(
    plugin = crate::rendering::billboard::BillboardPlugin,
    schedule = PostUpdate,
    config(after = bevy::transform::TransformSystems::Propagate)
)]
pub fn billboard_rotation_system(
    camera_query: Query<&Transform, ActiveCameraFilter>,
    mut billboard_query: Query<&mut Transform, WorldBillboardFilter>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return; // No active camera yet (or ambiguous), skip this frame
    };

    for mut billboard_transform in billboard_query.iter_mut() {
        billboard_transform.rotation = camera_transform.rotation;
    }
}

/// System that faces preview-layer billboards at the equipment-window preview
/// camera. No-op until the preview camera exists (empty query short-circuits).
#[auto_add_system(
    plugin = crate::rendering::billboard::BillboardPlugin,
    schedule = PostUpdate,
    config(after = bevy::transform::TransformSystems::Propagate)
)]
pub fn preview_billboard_rotation_system(
    camera_query: Query<&Transform, (With<EquipmentPreviewCamera>, Without<Billboard>)>,
    mut billboard_query: Query<&mut Transform, PreviewBillboardFilter>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return; // No preview camera, skip this frame
    };

    for mut billboard_transform in billboard_query.iter_mut() {
        billboard_transform.rotation = camera_transform.rotation;
    }
}

/// Plugin that registers billboard systems and resources
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct BillboardPlugin;
