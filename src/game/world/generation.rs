use bevy::prelude::*;

use super::{
    block::Block,
    chunk::{CHUNK_HEIGHT, CHUNK_SIZE, Chunk},
};

const MIN_HEIGHT: i32 = 5;
const SEA_HEIGHT: i32 = 8;
const MAX_HEIGHT: i32 = CHUNK_HEIGHT as i32 - 4;

pub fn generate_chunk(coord: IVec2) -> Chunk {
    let mut chunk = Chunk::empty();

    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let world_x = coord.x * CHUNK_SIZE as i32 + x as i32;
            let world_z = coord.y * CHUNK_SIZE as i32 + z as i32;
            let height = terrain_height(world_x, world_z);
            let sand = is_sand_column(world_x, world_z, height);

            for y in 0..=height {
                let block = block_for_layer(y, height, sand);
                chunk.set(x, y as usize, z, block);
            }
        }
    }

    chunk
}

fn terrain_height(x: i32, z: i32) -> i32 {
    let plains = octave_noise(x, z, 0.035, 3, 0.55) * 5.5;
    let hills = octave_noise(x + 812, z - 431, 0.012, 4, 0.5) * 11.0;
    let detail = octave_noise(x - 93, z + 211, 0.09, 2, 0.45) * 1.4;
    let height = 9.0 + plains + hills.max(0.0) * 0.55 + detail;

    height.round().clamp(MIN_HEIGHT as f32, MAX_HEIGHT as f32) as i32
}

fn is_sand_column(x: i32, z: i32, height: i32) -> bool {
    if height <= SEA_HEIGHT + 1 {
        return true;
    }

    let patches = octave_noise(x + 1297, z - 912, 0.055, 2, 0.48);
    patches > 0.48 && height <= SEA_HEIGHT + 4
}

fn block_for_layer(y: i32, height: i32, sand: bool) -> Block {
    if sand && y >= height - 2 {
        return Block::Sand;
    }

    if y == height {
        return Block::Grass;
    }

    if y >= height - 3 {
        return Block::Dirt;
    }

    Block::Stone
}

fn octave_noise(x: i32, z: i32, scale: f32, octaves: usize, persistence: f32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = scale;
    let mut max = 0.0;

    for _ in 0..octaves {
        total += value_noise(x as f32 * frequency, z as f32 * frequency) * amplitude;
        max += amplitude;
        amplitude *= persistence;
        frequency *= 2.0;
    }

    total / max
}

fn value_noise(x: f32, z: f32) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let z1 = z0 + 1;
    let sx = smooth(x - x0 as f32);
    let sz = smooth(z - z0 as f32);
    let a = random_cell(x0, z0);
    let b = random_cell(x1, z0);
    let c = random_cell(x0, z1);
    let d = random_cell(x1, z1);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;

    (ab + (cd - ab) * sz) * 2.0 - 1.0
}

fn smooth(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn random_cell(x: i32, z: i32) -> f32 {
    let mut value =
        (x as u32).wrapping_mul(374_761_393) ^ (z as u32).wrapping_mul(668_265_263) ^ 0x9e37_79b9;
    value = (value ^ (value >> 13)).wrapping_mul(1_274_126_177);
    ((value ^ (value >> 16)) & 0xffff) as f32 / 65_535.0
}
