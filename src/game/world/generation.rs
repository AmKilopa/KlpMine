use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use super::{
    block::Block,
    chunk::{CHUNK_SIZE, Chunk, local_in_bounds},
};

const CACHE_MARGIN: usize = 4;
const CACHE_SIZE: usize = CHUNK_SIZE + CACHE_MARGIN * 2;

const MIN_HEIGHT: i32 = 18;
const SEA_HEIGHT: i32 = 22;
const MAX_HEIGHT: i32 = 90;

#[derive(Resource, Clone, Copy)]
pub struct WorldSeed {
    pub value: u64,
}

pub fn new_world_seed() -> WorldSeed {
    let time_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64 ^ duration.as_secs().rotate_left(32))
        .unwrap_or(0x8f5d_2b31_a49c_760d);

    WorldSeed {
        value: mix_u64(time_seed),
    }
}

pub fn terrain_height_at(x: i32, z: i32, seed: u64) -> i32 {
    terrain_height(x, z, seed)
}

pub fn player_spawn_position(seed: u64) -> Vec3 {
    let spawn = spawn_column(seed);
    let y = terrain_height_at(spawn.x, spawn.y, seed) as f32 + 1.02;
    Vec3::new(spawn.x as f32 + 0.5, y, spawn.y as f32 + 0.5)
}

pub fn generate_chunk(coord: IVec2, seed: u64) -> Chunk {
    let mut chunk = Chunk::empty();
    let ox = coord.x * CHUNK_SIZE as i32;
    let oz = coord.y * CHUNK_SIZE as i32;

    let mut heights = [[0i32; CACHE_SIZE]; CACHE_SIZE];
    let mut sands = [[false; CACHE_SIZE]; CACHE_SIZE];

    for cx in 0..CACHE_SIZE {
        for cz in 0..CACHE_SIZE {
            let wx = ox - CACHE_MARGIN as i32 + cx as i32;
            let wz = oz - CACHE_MARGIN as i32 + cz as i32;
            let h = terrain_height(wx, wz, seed);
            heights[cx][cz] = h;
            sands[cx][cz] = is_sand_column(wx, wz, h, seed);
        }
    }

    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let h = heights[x + CACHE_MARGIN][z + CACHE_MARGIN];
            let sand = sands[x + CACHE_MARGIN][z + CACHE_MARGIN];

            for y in 0..=h as usize {
                chunk.set(x, y, z, block_for_layer(y as i32, h, sand));
            }

            if h < SEA_HEIGHT {
                for y in h + 1..=SEA_HEIGHT {
                    chunk.set(x, y as usize, z, Block::Water);
                }
            }
        }
    }

    generate_trees_batch(coord, &mut chunk, seed, &heights, &sands);
    carve_caves_batch(coord, &mut chunk, seed, &heights);
    chunk
}

fn generate_trees_batch(
    coord: IVec2,
    chunk: &mut Chunk,
    seed: u64,
    heights: &[[i32; CACHE_SIZE]; CACHE_SIZE],
    sands: &[[bool; CACHE_SIZE]; CACHE_SIZE],
) {
    let ox = coord.x * CHUNK_SIZE as i32;
    let oz = coord.y * CHUNK_SIZE as i32;
    let base_x = ox - CACHE_MARGIN as i32;
    let base_z = oz - CACHE_MARGIN as i32;

    for wx in ox - 4..ox + CHUNK_SIZE as i32 + 4 {
        for wz in oz - 4..oz + CHUNK_SIZE as i32 + 4 {
            if !is_tree_center(wx, wz, seed) {
                continue;
            }

            let cx = (wx - base_x) as usize;
            let cz = (wz - base_z) as usize;
            let ground = heights[cx][cz];
            if sands[cx][cz] || ground < SEA_HEIGHT + 2 {
                continue;
            }

            place_tree(chunk, ox, oz, wx, ground + 1, wz, tree_height(wx, wz, seed));
        }
    }
}

fn place_tree(chunk: &mut Chunk, ox: i32, oz: i32, x: i32, y: i32, z: i32, height: i32) {
    for offset_y in 0..height {
        place_in_chunk(
            chunk,
            ox,
            oz,
            IVec3::new(x, y + offset_y, z),
            Block::Log,
            false,
        );
    }

    let crown_y = y + height - 1;

    for dy in -1..=1 {
        let radius: i32 = if dy == 1 { 1 } else { 2 };
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() + dz.abs() > radius + 1 {
                    continue;
                }
                let pos = IVec3::new(x + dx, crown_y + dy, z + dz);
                if pos.y <= y || (dx == 0 && dz == 0 && dy <= 0) {
                    continue;
                }
                place_in_chunk(chunk, ox, oz, pos, Block::Leaves, true);
            }
        }
    }
}

fn place_in_chunk(chunk: &mut Chunk, ox: i32, oz: i32, pos: IVec3, block: Block, air_only: bool) {
    let local = IVec3::new(pos.x - ox, pos.y, pos.z - oz);
    if !local_in_bounds(local.x, local.y, local.z) {
        return;
    }
    if air_only && chunk.get(local.x, local.y, local.z).is_solid() {
        return;
    }
    chunk.set(local.x as usize, local.y as usize, local.z as usize, block);
}

fn is_tree_center(x: i32, z: i32, seed: u64) -> bool {
    let spacing = if random_cell(x / 7, z / 7, salted_seed(seed, 0xd1e2_f3a4_b5c6_d7e8)) > 0.6 {
        9
    } else {
        13
    };
    x.rem_euclid(spacing) == 0
        && z.rem_euclid(spacing) == 0
        && random_cell(x / spacing, z / spacing, salted_seed(seed, 0x1f84_2f0b_5a0d_b31a)) > 0.7
}

fn tree_height(x: i32, z: i32, seed: u64) -> i32 {
    5 + (random_cell(x + 41, z - 19, salted_seed(seed, 0x739b_12dd_58c7_e4f1)) * 3.0).floor() as i32
}

fn terrain_height(x: i32, z: i32, seed: u64) -> i32 {
    let warp_x = octave_noise(
        x + 531,
        z - 917,
        0.018,
        2,
        0.55,
        salted_seed(seed, 0xb08c_3d92_6f51_c2aa),
    ) * 18.0;
    let warp_z = octave_noise(
        x - 1237,
        z + 349,
        0.018,
        2,
        0.55,
        salted_seed(seed, 0xf793_0ac5_34e6_184b),
    ) * 18.0;
    let nx = x + warp_x.round() as i32;
    let nz = z + warp_z.round() as i32;
    let continent = octave_noise(
        nx - 1700,
        nz + 900,
        0.005,
        4,
        0.52,
        salted_seed(seed, 0x4634_a1d0_9cbe_8b27),
    ) * 16.0;
    let plains = octave_noise(
        nx,
        nz,
        0.032,
        3,
        0.55,
        salted_seed(seed, 0x98af_2e70_c4d1_5d3c),
    ) * 3.5;
    let hills = octave_noise(
        nx + 812,
        nz - 431,
        0.014,
        3,
        0.5,
        salted_seed(seed, 0x0ad3_b942_1e7c_a810),
    )
    .max(0.0)
        * 12.0;
    let ridges = ((octave_noise(
        nx + 3201,
        nz - 1902,
        0.028,
        2,
        0.5,
        salted_seed(seed, 0x1a2b_3c4d_5e6f_7890),
    )
    .abs()
        * 2.0
        - 0.3)
        .max(0.0)
        * 2.4)
        .max(0.0);
    let detail = octave_noise(
        nx - 93,
        nz + 211,
        0.07,
        2,
        0.5,
        salted_seed(seed, 0xe38f_d47b_17cc_6390),
    ) * 1.0;
    let valley = (1.0 - octave_noise(
        nx + 4102,
        nz - 3107,
        0.009,
        2,
        0.5,
        salted_seed(seed, 0xabcd_ef01_2345_6789),
    ).abs())
        .max(0.0)
        * 3.0;
    let height = 28.0 + continent + plains + hills + ridges + detail - valley;

    height.round().clamp(MIN_HEIGHT as f32, MAX_HEIGHT as f32) as i32
}

fn is_sand_column(x: i32, z: i32, height: i32, seed: u64) -> bool {
    let basin = octave_noise(
        x - 2400,
        z + 1700,
        0.016,
        3,
        0.52,
        salted_seed(seed, 0x43f9_e1a8_32b4_7c0d),
    );
    let shore = octave_noise(
        x + 1297,
        z - 912,
        0.06,
        2,
        0.48,
        salted_seed(seed, 0x6518_c79a_3b0e_f2d4),
    );
    if height <= SEA_HEIGHT {
        return true;
    }
    let wet_edge = height == SEA_HEIGHT + 1 && (basin < 0.08 || shore > -0.22);
    let dry_edge = height == SEA_HEIGHT + 2 && basin < -0.18 && shore > 0.2;

    wet_edge || dry_edge
}

fn spawn_column(seed: u64) -> IVec2 {
    let mut fallback = IVec2::new(0, 8);
    let mut fallback_score = i32::MAX;

    for radius in 0i32..=28 {
        for x in -radius..=radius {
            for z in -radius..=radius {
                if x.abs() != radius && z.abs() != radius {
                    continue;
                }

                let height = terrain_height(x, z, seed);
                let score = x.abs() + z.abs();

                if height > SEA_HEIGHT && score < fallback_score {
                    fallback = IVec2::new(x, z);
                    fallback_score = score;
                }

                if height >= SEA_HEIGHT + 2
                    && !is_sand_column(x, z, height, seed)
                    && !tree_near_spawn(x, z, seed)
                {
                    return IVec2::new(x, z);
                }
            }
        }
    }

    fallback
}

fn tree_near_spawn(x: i32, z: i32, seed: u64) -> bool {
    for tx in x - 3..=x + 3 {
        for tz in z - 3..=z + 3 {
            if is_tree_center(tx, tz, seed) {
                return true;
            }
        }
    }
    false
}

fn is_sand_column_local(chunk: &Chunk, x: usize, z: usize, height: usize) -> bool {
    if height <= SEA_HEIGHT as usize {
        return true;
    }
    let top_block = chunk.get(x as i32, height as i32, z as i32);
    top_block == Block::Sand || chunk.get(x as i32, (height - 1) as i32, z as i32) == Block::Sand
}

fn carve_caves_batch(
    coord: IVec2,
    chunk: &mut Chunk,
    seed: u64,
    heights: &[[i32; CACHE_SIZE]; CACHE_SIZE],
) {
    let ox = coord.x * CHUNK_SIZE as i32;
    let oz = coord.y * CHUNK_SIZE as i32;
    let cave_seed = salted_seed(seed, 0xcafe_cafe_cafe_cafe);

    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let wx = ox + x as i32;
            let wz = oz + z as i32;
            let height = heights[x + CACHE_MARGIN][z + CACHE_MARGIN];
            if height < SEA_HEIGHT + 1 { continue; }
            if is_sand_column_local(chunk, x, z, height as usize) { continue; }

            let density = octave_noise(wx + 5000, wz - 3000, 0.012, 3, 0.5, cave_seed);
            if density < 0.1 || density > 0.6 { continue; }

            let top = height as usize;

            for y in 3..top {
                let block = chunk.get(x as i32, y as i32, z as i32);
                if !block.is_solid() { continue; }

                let cave_3d = octave_noise_3d(wx, y as i32, wz, 0.055, 2, 0.5, cave_seed);

                let surface_dist = height - y as i32;
                let entrance_boost = if surface_dist <= 3 {
                    (4 - surface_dist) as f32 * 0.08
                } else {
                    0.0
                };

                let threshold = 0.48 - density * 0.25;

                if cave_3d + entrance_boost > threshold {
                    chunk.set(x, y, z, Block::Air);
                }
            }
        }
    }
}

fn octave_noise_3d(x: i32, y: i32, z: i32, scale: f32, octaves: usize, persistence: f32, seed: u64) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = scale;
    let mut max = 0.0;

    for octave in 0..octaves {
        total += value_noise_3d(
            x as f32 * frequency,
            y as f32 * frequency,
            z as f32 * frequency,
            salted_seed(seed, octave as u64),
        ) * amplitude;
        max += amplitude;
        amplitude *= persistence;
        frequency *= 2.0;
    }

    total / max
}

fn value_noise_3d(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let sx = smooth(x - x0 as f32);
    let sy = smooth(y - y0 as f32);
    let sz = smooth(z - z0 as f32);

    let a000 = random_cell_3d(x0, y0, z0, seed);
    let a100 = random_cell_3d(x0 + 1, y0, z0, seed);
    let a010 = random_cell_3d(x0, y0 + 1, z0, seed);
    let a110 = random_cell_3d(x0 + 1, y0 + 1, z0, seed);
    let a001 = random_cell_3d(x0, y0, z0 + 1, seed);
    let a101 = random_cell_3d(x0 + 1, y0, z0 + 1, seed);
    let a011 = random_cell_3d(x0, y0 + 1, z0 + 1, seed);
    let a111 = random_cell_3d(x0 + 1, y0 + 1, z0 + 1, seed);

    let c00 = a000 + (a100 - a000) * sx;
    let c10 = a010 + (a110 - a010) * sx;
    let c01 = a001 + (a101 - a001) * sx;
    let c11 = a011 + (a111 - a011) * sx;

    let c0 = c00 + (c10 - c00) * sy;
    let c1 = c01 + (c11 - c01) * sy;

    (c0 + (c1 - c0) * sz) * 2.0 - 1.0
}

fn random_cell_3d(x: i32, y: i32, z: i32, seed: u64) -> f32 {
    let value = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (y as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ (z as i64 as u64).wrapping_mul(0x6e23_419f_8b3c_d1ea);
    let mixed = mix_u64(value);
    ((mixed >> 40) as u32) as f32 / 16_777_215.0
}

fn block_for_layer(y: i32, height: i32, sand: bool) -> Block {
    if sand && y >= height - 1 {
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

fn octave_noise(x: i32, z: i32, scale: f32, octaves: usize, persistence: f32, seed: u64) -> f32 {
    let mut total = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = scale;
    let mut max = 0.0f32;

    for octave in 0..octaves {
        total += value_noise(
            x as f32 * frequency,
            z as f32 * frequency,
            salted_seed(seed, octave as u64),
        ) * amplitude;
        max += amplitude;
        amplitude *= persistence;
        frequency *= 2.0;
    }

    total / max
}

fn value_noise(x: f32, z: f32, seed: u64) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let sx = smooth(x - x0 as f32);
    let sz = smooth(z - z0 as f32);
    let a = random_cell(x0, z0, seed);
    let b = random_cell(x0 + 1, z0, seed);
    let c = random_cell(x0, z0 + 1, seed);
    let d = random_cell(x0 + 1, z0 + 1, seed);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;

    (ab + (cd - ab) * sz) * 2.0 - 1.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn random_cell(x: i32, z: i32, seed: u64) -> f32 {
    let value = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (z as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    let mixed = mix_u64(value);
    ((mixed >> 40) as u32) as f32 / 16_777_215.0
}

fn salted_seed(seed: u64, salt: u64) -> u64 {
    mix_u64(seed ^ salt)
}

fn mix_u64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_is_stable_for_same_seed() {
        let seed = 123_456_789;

        assert_eq!(
            terrain_height_at(-12, 34, seed),
            terrain_height_at(-12, 34, seed)
        );
        assert_eq!(player_spawn_position(seed), player_spawn_position(seed));
    }

    #[test]
    fn terrain_changes_between_seeds() {
        let first = (0..24)
            .map(|x| terrain_height_at(x, 11, 111))
            .collect::<Vec<_>>();
        let second = (0..24)
            .map(|x| terrain_height_at(x, 11, 222))
            .collect::<Vec<_>>();

        assert_ne!(first, second);
    }
}
