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
    let Some(hit) = raycast_blocks_mut(camera.translation, *camera.forward(), &mut chunks) else {
        return;
    };

    let mut changed_origin = None;
    let mut changed_local = None;

    for (mut chunk, transform, _) in &mut chunks {
        let local = hit.block - transform.translation().floor().as_ivec3();

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

    let Some(hit) = raycast_blocks(camera.translation, *camera.forward(), &chunks) else {
        return;
    };

    gizmos.cube(
        Transform::from_translation(hit.block.as_vec3() + Vec3::splat(0.5))
            .with_scale(Vec3::splat(1.015)),
        Color::srgba(0.02, 0.02, 0.02, 0.95),
    );
}

fn raycast_blocks(
    origin: Vec3,
    direction: Vec3,
    chunks: &Query<(&Chunk, &GlobalTransform)>,
) -> Option<BlockHit> {
    voxel_raycast(origin, direction, BLOCK_REACH, |block_pos| {
        block_at(block_pos, chunks).is_solid()
    })
}

fn raycast_blocks_mut(
    origin: Vec3,
    direction: Vec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Option<BlockHit> {
    voxel_raycast(origin, direction, BLOCK_REACH, |block_pos| {
        for (chunk, transform, _) in chunks.iter() {
            let local = block_pos - transform.translation().floor().as_ivec3();

            if chunk.get(local.x, local.y, local.z).is_solid() {
                return true;
            }
        }

        false
    })
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

#[derive(Clone, Copy)]
struct BlockHit {
    block: IVec3,
}

fn voxel_raycast(
    origin: Vec3,
    direction: Vec3,
    reach: f32,
    mut is_solid: impl FnMut(IVec3) -> bool,
) -> Option<BlockHit> {
    let direction = direction.normalize_or_zero();

    if direction == Vec3::ZERO {
        return None;
    }

    let mut block = origin.floor().as_ivec3();
    let step = IVec3::new(
        axis_step(direction.x),
        axis_step(direction.y),
        axis_step(direction.z),
    );
    let mut t_max = Vec3::new(
        first_axis_distance(origin.x, direction.x, step.x),
        first_axis_distance(origin.y, direction.y, step.y),
        first_axis_distance(origin.z, direction.z, step.z),
    );
    let t_delta = Vec3::new(
        axis_delta(direction.x),
        axis_delta(direction.y),
        axis_delta(direction.z),
    );
    let mut traveled = 0.0;

    while traveled <= reach {
        if is_solid(block) {
            return Some(BlockHit { block });
        }

        if t_max.x <= t_max.y && t_max.x <= t_max.z {
            block.x += step.x;
            traveled = t_max.x;
            t_max.x += t_delta.x;
        } else if t_max.y <= t_max.z {
            block.y += step.y;
            traveled = t_max.y;
            t_max.y += t_delta.y;
        } else {
            block.z += step.z;
            traveled = t_max.z;
            t_max.z += t_delta.z;
        }
    }

    None
}

fn axis_step(value: f32) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn first_axis_distance(origin: f32, direction: f32, step: i32) -> f32 {
    if step > 0 {
        ((origin.floor() + 1.0) - origin) / direction
    } else if step < 0 {
        (origin - origin.floor()) / -direction
    } else {
        f32::INFINITY
    }
}

fn axis_delta(direction: f32) -> f32 {
    if direction == 0.0 {
        f32::INFINITY
    } else {
        (1.0 / direction).abs()
    }
}
