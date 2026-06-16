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
            let mut x = 0;

            while x < CHUNK_SIZE as i32 {
                let local = IVec3::new(x, y, z);
                if !needs_collision(chunk, local, &block_at) {
                    x += 1;
                    continue;
                }

                let start = x;
                x += 1;

                while x < CHUNK_SIZE as i32 {
                    let local = IVec3::new(x, y, z);
                    if !needs_collision(chunk, local, &block_at) {
                        break;
                    }
                    x += 1;
                }

                let length = x - start;
                shapes.push((
                    Vec3::new(
                        start as f32 + length as f32 * 0.5,
                        y as f32 + BLOCK_HALF,
                        z as f32 + BLOCK_HALF,
                    ),
                    Quat::IDENTITY,
                    Collider::cuboid(length as f32 * 0.5, BLOCK_HALF, BLOCK_HALF),
                ));
            }
        }
    }

    Collider::compound(shapes)
}

fn needs_collision(chunk: &Chunk, local: IVec3, block_at: &impl Fn(IVec3) -> Block) -> bool {
    if !chunk.get(local.x, local.y, local.z).is_solid() {
        return false;
    }

    !SOLID_NEIGHBORS
        .into_iter()
        .all(|offset| block_at(local + offset).is_solid())
}

const SOLID_NEIGHBORS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];
