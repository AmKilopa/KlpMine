use bevy::prelude::*;

use super::block::Block;

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_HEIGHT: usize = 48;
const BLOCK_COUNT: usize = CHUNK_SIZE * CHUNK_HEIGHT * CHUNK_SIZE;

#[derive(Clone, Component)]
pub struct Chunk {
    pub blocks: Box<[Block; BLOCK_COUNT]>,
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: Box::new([Block::Air; BLOCK_COUNT]),
        }
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Block {
        if xz_outside(x, z) || y < 0 || y >= CHUNK_HEIGHT as i32 {
            return Block::Air;
        }

        self.blocks[index(x as usize, y as usize, z as usize)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, block: Block) {
        self.blocks[index(x, y, z)] = block;
    }

    pub fn set_local(&mut self, local: IVec3, block: Block) {
        if xz_outside(local.x, local.z) || local.y < 0 || local.y >= CHUNK_HEIGHT as i32 {
            return;
        }

        self.set(local.x as usize, local.y as usize, local.z as usize, block);
    }
}

pub fn xz_outside(x: i32, z: i32) -> bool {
    x < 0 || z < 0 || x >= CHUNK_SIZE as i32 || z >= CHUNK_SIZE as i32
}

fn index(x: usize, y: usize, z: usize) -> usize {
    y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x
}
