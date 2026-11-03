//! Sage's persistent per-cell geometry, following roBrowserLegacy's
//! PropertyGround.js and LPEffect.js (EF_BOTTOM_VO/DE/VI/LA).
//! Cell parenting owns lifetime, including partial removal and zone changes.
//! NOTE: Cells inherit group height; add per-cell grounding for sloped fields.

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use lifthrasir_data::EffectDescriptor;
use lifthrasir_runtime::rendering::coordinates::RO_UNITS_PER_CELL;

const TEXTURES: [&str; 4] = [
    "ro://data/texture/effect/ring_red.tga",
    "ro://data/texture/effect/ring_blue.tga",
    "ro://data/texture/effect/ring_yellow.tga",
    "ro://data/texture/effect/aaa copy.bmp",
];
const LAND_PROTECTOR: usize = 3;

#[derive(Component)]
pub(super) struct SageField {
    index: usize,
    phase: f32,
}

/// Queue geometry only for the four Sage field keys. Its mesh is initialized
/// after skill-unit spawning, without changing the shared visual spawner's API.
pub(super) fn spawn(
    commands: &mut Commands,
    descriptor: &EffectDescriptor,
    parent: Entity,
) -> bool {
    let index = match descriptor.bespoke_key() {
        Some("sage_volcano") => 0,
        Some("sage_deluge") => 1,
        Some("sage_violent_gale") => 2,
        Some("sage_land_protector") => LAND_PROTECTOR,
        _ => return false,
    };
    commands.spawn((
        SageField {
            index,
            phase: (parent.index_u32() % 16) as f32 * TAU / 16.0,
        },
        Transform::from_xyz(0.0, 0.05, 0.0),
        Visibility::default(),
        ChildOf(parent),
    ));
    true
}

/// Share GPU assets across cells and casts instead of duplicating a material
/// and mesh for every cell of a 121-cell Land Protector.
#[derive(Default)]
pub(super) struct SageFieldAssets {
    meshes: Option<[Handle<Mesh>; 2]>,
    materials: Option<[Handle<SageFieldMaterial>; 4]>,
}

pub(super) fn initialize(
    mut commands: Commands,
    fields: Query<(Entity, &SageField), Without<Mesh3d>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SageFieldMaterial>>,
    mut cached: Local<SageFieldAssets>,
) {
    let cached = &mut *cached;
    for (entity, field) in &fields {
        let meshes = cached.meshes.get_or_insert_with(|| {
            [
                meshes.add(element_cone()),
                meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(RO_UNITS_PER_CELL))),
            ]
        });
        let materials = cached.materials.get_or_insert_with(|| {
            std::array::from_fn(|index| {
                materials.add(SageFieldMaterial {
                    texture: asset_server.load(TEXTURES[index]),
                    opacity: if index == LAND_PROTECTOR { 0.7 } else { 0.42 },
                })
            })
        });
        commands.entity(entity).insert((
            Mesh3d(meshes[usize::from(field.index == LAND_PROTECTOR)].clone()),
            MeshMaterial3d(materials[field.index].clone()),
            NotShadowCaster,
        ));
    }
}

/// Reproduce the reference's pulsing radii and rotating elemental cones. These
/// are animation clocks only; the server, not these clocks, removes the cells.
pub(super) fn animate(time: Res<Time>, mut fields: Query<(&SageField, &mut Transform)>) {
    let tick = time.elapsed_secs() * 1_000.0;
    for (field, mut transform) in &mut fields {
        if field.index == LAND_PROTECTOR {
            let phase = if field.phase < PI { 0.0 } else { PI };
            transform.scale = Vec3::splat(0.8 + 0.05 * (phase + tick / (540.0 * PI)).sin());
            continue;
        }
        let radius = (tick / (360.0 * PI) + field.phase).sin().max(0.5);
        transform.scale = Vec3::new(radius, 1.0, radius);
        transform.rotation = Quat::from_rotation_y(field.phase + tick * PI / 1_440.0);
    }
}

/// An open, 20-segment cone with staggered upper vertices, matching the
/// reference's top radius 3, bottom radius 1 and height 2 in cell units.
fn element_cone() -> Mesh {
    const SEGMENTS: u32 = 20;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for segment in 0..=SEGMENTS {
        let u = segment as f32 / SEGMENTS as f32;
        let bottom = u * TAU;
        let top = (u + 0.5 / SEGMENTS as f32) * TAU;
        positions.push([
            bottom.sin() * RO_UNITS_PER_CELL,
            0.0,
            bottom.cos() * RO_UNITS_PER_CELL,
        ]);
        positions.push([
            top.sin() * 3.0 * RO_UNITS_PER_CELL,
            2.0 * RO_UNITS_PER_CELL,
            top.cos() * 3.0 * RO_UNITS_PER_CELL,
        ]);
        uvs.extend([[u, 1.0], [u + 0.5 / SEGMENTS as f32, 0.0]]);
        if segment < SEGMENTS {
            let base = segment * 2;
            indices.extend([base, base + 1, base + 2, base + 1, base + 2, base + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// The reference discards dark pixels then uses fixed alpha, rather than
/// additive blending (which would wash out the textured field).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct SageFieldMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
    #[uniform(2)]
    opacity: f32,
}

impl Material for SageFieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "ro://shaders/sage_field.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
