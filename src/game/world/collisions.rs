use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::{
    block::Block,
    chunk::{CHUNK_HEIGHT, CHUNK_SIZE, Chunk},
};

const BLOCK_HALF: f32 = 0.5;

pub fn build_chunk_collider_with_neighbors(
    chunk: &Chunk,
    block_at: impl Fn(IVec3) -> Block,
) -> Collider {
    let mut shapes = Vec::new();

    for y in 0..CHUNK_HEIGHT as i32 {
        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                let block = chunk.get(x, y, z);
                if !block.is_solid() {
                    continue;
                }

                let local = IVec3::new(x, y, z);
                if SOLID_NEIGHBORS
                    .into_iter()
                    .all(|offset| block_at(local + offset).is_solid())
                {
                    continue;
                }

                shapes.push((
                    local.as_vec3() + Vec3::splat(BLOCK_HALF),
                    Quat::IDENTITY,
                    Collider::cuboid(BLOCK_HALF, BLOCK_HALF, BLOCK_HALF),
                ));
            }
        }
    }

    Collider::compound(shapes)
}

const SOLID_NEIGHBORS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];
