use bevy::prelude::*;

use super::{
    block::Block,
    chunk::{CHUNK_HEIGHT, CHUNK_SIZE, Chunk},
};

const FLAT_GROUND_HEIGHT: usize = 4;

pub fn generate_chunk(coord: IVec2) -> Chunk {
    let mut chunk = Chunk::empty();

    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let _world_x = coord.x * CHUNK_SIZE as i32 + x as i32;
            let _world_z = coord.y * CHUNK_SIZE as i32 + z as i32;
            let height = FLAT_GROUND_HEIGHT.min(CHUNK_HEIGHT - 1);

            for y in 0..=height {
                let block = if y == height {
                    Block::Grass
                } else {
                    Block::Dirt
                };

                chunk.set(x, y, z, block);
            }
        }
    }

    chunk
}
