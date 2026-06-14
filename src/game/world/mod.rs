use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::PrimitiveTopology};

use crate::game::camera::PlayerCamera;

mod block;
mod chunk;
mod generation;
mod materials;
mod meshing;

pub use chunk::Chunk;

use block::Block;
use chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use generation::generate_chunk;
use materials::BlockMaterials;
use meshing::build_chunk_mesh_with_neighbors;

pub struct WorldPlugin;

const BLOCK_REACH: f32 = 7.0;
const RAY_STEP: f32 = 0.04;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (materials::setup_materials, spawn_world).chain())
            .add_systems(
                Update,
                (break_selected_block, update_block_selection).chain(),
            );
    }
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    let radius = 5;
    let mut generated_chunks = Vec::new();

    for chunk_x in -radius..=radius {
        for chunk_z in -radius..=radius {
            let origin = IVec3::new(chunk_x * CHUNK_SIZE as i32, 0, chunk_z * CHUNK_SIZE as i32);

            generated_chunks.push((origin, generate_chunk(IVec2::new(chunk_x, chunk_z))));
        }
    }

    for (origin, chunk) in &generated_chunks {
        let Some(mesh) = build_chunk_mesh_with_neighbors(&chunk, |local| {
            block_from_snapshot(*origin + local, &generated_chunks)
        }) else {
            continue;
        };

        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.terrain.clone()),
            Transform::from_translation(origin.as_vec3()),
            chunk.clone(),
        ));
    }
}

fn break_selected_block(
    mouse: Res<ButtonInput<MouseButton>>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut chunks: Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };
    let Some(block_pos) = raycast_blocks_mut(camera.translation, *camera.forward(), &mut chunks)
    else {
        return;
    };

    let mut changed_origin = None;
    let mut changed_local = None;

    for (mut chunk, transform, _) in &mut chunks {
        let local = block_pos - transform.translation().floor().as_ivec3();

        if !is_inside_chunk(local) {
            continue;
        }

        chunk.set_local(local, Block::Air);
        changed_origin = Some(transform.translation().floor().as_ivec3());
        changed_local = Some(local);
        break;
    }

    let Some(origin) = changed_origin else {
        return;
    };
    let Some(local) = changed_local else {
        return;
    };

    let snapshot = chunk_snapshot(&chunks);

    for (chunk, transform, mut mesh_handle) in &mut chunks {
        let chunk_origin = transform.translation().floor().as_ivec3();

        if should_rebuild_chunk(origin, local, chunk_origin) {
            let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
                block_from_snapshot(chunk_origin + local, &snapshot)
            })
            .unwrap_or_else(empty_mesh);
            *mesh_handle = Mesh3d(meshes.add(mesh));
        }
    }
}

fn update_block_selection(
    cameras: Query<&Transform, With<PlayerCamera>>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };

    let Some(block) = raycast_blocks(camera.translation, *camera.forward(), &chunks) else {
        return;
    };

    gizmos.cube(
        Transform::from_translation(block.as_vec3() + Vec3::splat(0.5))
            .with_scale(Vec3::splat(1.015)),
        Color::srgba(0.02, 0.02, 0.02, 0.95),
    );
}

fn raycast_blocks(
    origin: Vec3,
    direction: Vec3,
    chunks: &Query<(&Chunk, &GlobalTransform)>,
) -> Option<IVec3> {
    let mut distance = 0.0;

    while distance <= BLOCK_REACH {
        let point = origin + direction * distance;
        let block_pos = point.floor().as_ivec3();

        if block_at(block_pos, chunks).is_solid() {
            return Some(block_pos);
        }

        distance += RAY_STEP;
    }

    None
}

fn raycast_blocks_mut(
    origin: Vec3,
    direction: Vec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Option<IVec3> {
    let mut distance = 0.0;

    while distance <= BLOCK_REACH {
        let point = origin + direction * distance;
        let block_pos = point.floor().as_ivec3();

        for (chunk, transform, _) in chunks.iter() {
            let local = block_pos - transform.translation().floor().as_ivec3();

            if chunk.get(local.x, local.y, local.z).is_solid() {
                return Some(block_pos);
            }
        }

        distance += RAY_STEP;
    }

    None
}

fn block_at(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> Block {
    block_at_world(world_pos, chunks)
}

pub fn is_solid_at(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> bool {
    block_at_world(world_pos, chunks).is_solid()
}

fn block_at_world(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> Block {
    for (chunk, transform) in chunks.iter() {
        let chunk_origin = transform.translation().floor().as_ivec3();
        let local = world_pos - chunk_origin;
        let block = chunk.get(local.x, local.y, local.z);

        if block.is_solid() {
            return block;
        }
    }

    Block::Air
}

fn is_inside_chunk(local: IVec3) -> bool {
    local.x >= 0
        && local.y >= 0
        && local.z >= 0
        && local.x < CHUNK_SIZE as i32
        && local.y < CHUNK_HEIGHT as i32
        && local.z < CHUNK_SIZE as i32
}

fn empty_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn should_rebuild_chunk(changed_origin: IVec3, changed_local: IVec3, chunk_origin: IVec3) -> bool {
    if chunk_origin == changed_origin {
        return true;
    }

    let chunk_size = CHUNK_SIZE as i32;

    (changed_local.x == 0 && chunk_origin == changed_origin + IVec3::new(-chunk_size, 0, 0))
        || (changed_local.x == chunk_size - 1
            && chunk_origin == changed_origin + IVec3::new(chunk_size, 0, 0))
        || (changed_local.z == 0 && chunk_origin == changed_origin + IVec3::new(0, 0, -chunk_size))
        || (changed_local.z == chunk_size - 1
            && chunk_origin == changed_origin + IVec3::new(0, 0, chunk_size))
}

fn chunk_snapshot(
    chunks: &Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Vec<(IVec3, Chunk)> {
    chunks
        .iter()
        .map(|(chunk, transform, _)| (transform.translation().floor().as_ivec3(), chunk.clone()))
        .collect()
}

fn block_from_snapshot(world_pos: IVec3, chunks: &[(IVec3, Chunk)]) -> Block {
    for (origin, chunk) in chunks {
        let local = world_pos - *origin;

        if is_inside_chunk(local) {
            return chunk.get(local.x, local.y, local.z);
        }
    }

    Block::Air
}
