use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use super::{
    block::Block,
    chunk::{CHUNK_HEIGHT, CHUNK_SIZE, Chunk},
};

const ATLAS_WIDTH: f32 = 272.0;
const ATLAS_HEIGHT: f32 = 34.0;
const CELL_SIZE: f32 = 34.0;
const UV_INSET: f32 = 0.5;
const WATER_MIN_SURFACE_HEIGHT: f32 = 0.12;
const WATER_MAX_SURFACE_HEIGHT: f32 = 1.0;

#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    fn add_face(&mut self, block: Block, origin: Vec3, face: Face) {
        let base = self.positions.len() as u32;
        let face_uvs = tile_uvs(face_tile(block, face.normal));
        let light = face_light(face.normal);

        for corner in face.corners {
            let p = origin + Vec3::from_array(corner);
            self.positions.push(p.into());
            self.normals.push([
                face.normal[0] as f32,
                face.normal[1] as f32,
                face.normal[2] as f32,
            ]);
            self.colors.push([light, light, light, 1.0]);
        }

        self.uvs.extend_from_slice(&face_uvs);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn add_water_face(&mut self, origin: Vec3, face: Face, fill: f32) {
        if face.normal == [0, -1, 0] {
            return;
        }

        let base = self.positions.len() as u32;
        let face_uvs = tile_uvs(Block::Water.atlas_index());
        let light = face_light(face.normal);
        let mut corners = face.corners;
        let surface = WATER_MIN_SURFACE_HEIGHT
            + fill.clamp(0.0, 1.0) * (WATER_MAX_SURFACE_HEIGHT - WATER_MIN_SURFACE_HEIGHT);

        for corner in &mut corners {
            if corner[1] > 0.0 {
                corner[1] = surface;
            }
        }

        for corner in corners {
            let p = origin + Vec3::from_array(corner);
            self.positions.push(p.into());
            self.normals.push([
                face.normal[0] as f32,
                face.normal[1] as f32,
                face.normal[2] as f32,
            ]);
            self.colors.push([light, light, light, 1.0]);
        }

        self.uvs.extend_from_slice(&face_uvs);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn add_shadow_quad(&mut self, origin: Vec3, alpha: f32) {
        let base = self.positions.len() as u32;
        let alpha = alpha.clamp(0.0, 1.0);

        self.positions.extend_from_slice(&[
            [origin.x, origin.y, origin.z + 1.0],
            [origin.x + 1.0, origin.y, origin.z + 1.0],
            [origin.x + 1.0, origin.y, origin.z],
            [origin.x, origin.y, origin.z],
        ]);
        self.normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        self.uvs
            .extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        self.colors.extend_from_slice(&[[1.0, 1.0, 1.0, alpha]; 4]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn build(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

pub fn build_chunk_mesh_with_neighbors(
    chunk: &Chunk,
    block_at: impl Fn(IVec3) -> Block,
) -> Option<Mesh> {
    build_chunk_layer_mesh_with_neighbors(chunk, block_at, |_| 1.0, MeshLayer::Solid)
}

pub fn build_chunk_water_mesh_with_neighbors(
    chunk: &Chunk,
    block_at: impl Fn(IVec3) -> Block,
    water_fill: impl Fn(IVec3) -> f32,
) -> Option<Mesh> {
    build_chunk_layer_mesh_with_neighbors(chunk, block_at, water_fill, MeshLayer::Water)
}

fn build_chunk_layer_mesh_with_neighbors(
    chunk: &Chunk,
    block_at: impl Fn(IVec3) -> Block,
    water_fill: impl Fn(IVec3) -> f32,
    layer: MeshLayer,
) -> Option<Mesh> {
    let mut builder = MeshBuilder::default();

    for y in 0..CHUNK_HEIGHT as i32 {
        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                let block = chunk.get(x, y, z);
                if !layer.contains(block) {
                    continue;
                }
                let local = IVec3::new(x, y, z);
                for face in FACES {
                    let neighbor = block_at(local + IVec3::from_array(face.normal));
                    if layer.should_draw_face(block, neighbor) {
                        match layer {
                            MeshLayer::Solid => builder.add_face(block, local.as_vec3(), face),
                            MeshLayer::Water => {
                                builder.add_water_face(local.as_vec3(), face, water_fill(local))
                            }
                        }
                    }
                }
            }
        }
    }

    (!builder.is_empty()).then(|| builder.build())
}

#[derive(Clone, Copy)]
enum MeshLayer {
    Solid,
    Water,
}

impl MeshLayer {
    fn contains(self, block: Block) -> bool {
        match self {
            Self::Solid => block.is_solid(),
            Self::Water => block.is_fluid(),
        }
    }

    fn should_draw_face(self, block: Block, neighbor: Block) -> bool {
        match self {
            Self::Solid => !neighbor.is_visible() || (block.is_solid() && !neighbor.is_solid()),
            Self::Water => neighbor == Block::Air,
        }
    }
}

pub fn build_item_mesh(block: Block) -> Mesh {
    let mut builder = MeshBuilder::default();
    for face in FACES {
        builder.add_face(block, Vec3::splat(-0.5), face);
    }
    builder.build()
}

pub fn build_log_stack_mesh(height: i32) -> Mesh {
    let mut builder = MeshBuilder::default();
    let height = height.max(1);
    let offset = height as f32 * 0.5;

    for y in 0..height {
        for face in FACES {
            builder.add_face(Block::Log, Vec3::new(-0.5, y as f32 - offset, -0.5), face);
        }
    }

    builder.build()
}

pub fn build_tree_shadow_mesh(chunk: &Chunk) -> Option<Mesh> {
    let mut builder = MeshBuilder::default();

    for z in 0..CHUNK_SIZE as i32 {
        for x in 0..CHUNK_SIZE as i32 {
            let Some(surface_y) = shadow_surface_y(chunk, x, z) else {
                continue;
            };
            let strength = canopy_shadow_strength(chunk, x, surface_y, z);
            if strength < 0.16 {
                continue;
            }
            builder.add_shadow_quad(
                Vec3::new(x as f32, surface_y as f32 + 1.012, z as f32),
                strength,
            );
        }
    }

    (!builder.is_empty()).then(|| builder.build())
}

fn shadow_surface_y(chunk: &Chunk, x: i32, z: i32) -> Option<i32> {
    (0..CHUNK_HEIGHT as i32).rev().find(|&y| {
        matches!(
            chunk.get(x, y, z),
            Block::Grass | Block::Dirt | Block::Sand | Block::Stone
        )
    })
}

fn canopy_shadow_strength(chunk: &Chunk, x: i32, surface_y: i32, z: i32) -> f32 {
    let mut strength = 0.0f32;

    for dz in -3i32..=3 {
        for dx in -3i32..=3 {
            let sample_x = x + dx;
            let sample_z = z + dz;
            if sample_x < 0
                || sample_z < 0
                || sample_x >= CHUNK_SIZE as i32
                || sample_z >= CHUNK_SIZE as i32
            {
                continue;
            }

            let distance = ((dx * dx + dz * dz) as f32).sqrt();
            if distance > 3.25 {
                continue;
            }

            for y in surface_y + 2..=(surface_y + 8).min(CHUNK_HEIGHT as i32 - 1) {
                if chunk.get(sample_x, y, sample_z) == Block::Leaves {
                    strength = strength.max((1.0 - distance / 3.7) * 0.72);
                    break;
                }
            }
        }
    }

    strength.clamp(0.0, 0.72)
}

fn face_light(normal: [i32; 3]) -> f32 {
    match normal {
        [0, 1, 0] => 1.0,
        [0, -1, 0] => 0.84,
        [1, 0, 0] => 0.99,
        [-1, 0, 0] => 0.96,
        [0, 0, 1] => 0.99,
        [0, 0, -1] => 0.95,
        _ => 0.96,
    }
}

fn face_tile(block: Block, normal: [i32; 3]) -> usize {
    match block {
        Block::Grass if normal == [0, 1, 0] => 1,
        Block::Grass if normal == [0, -1, 0] => 0,
        Block::Grass => 2,
        _ => block.atlas_index(),
    }
}

fn tile_uvs(index: usize) -> [[f32; 2]; 4] {
    let i = index as f32;
    let u0 = (i * CELL_SIZE + 1.0 + UV_INSET) / ATLAS_WIDTH;
    let u1 = (i * CELL_SIZE + 33.0 - UV_INSET) / ATLAS_WIDTH;
    let v0 = (1.0 + UV_INSET) / ATLAS_HEIGHT;
    let v1 = (33.0 - UV_INSET) / ATLAS_HEIGHT;

    [[u0, v1], [u1, v1], [u1, v0], [u0, v0]]
}

#[derive(Clone, Copy)]
struct Face {
    normal: [i32; 3],
    corners: [[f32; 3]; 4],
}

const FACES: [Face; 6] = [
    Face {
        normal: [1, 0, 0],
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
    },
    Face {
        normal: [-1, 0, 0],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
    },
    Face {
        normal: [0, 1, 0],
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
    },
    Face {
        normal: [0, -1, 0],
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
    },
    Face {
        normal: [0, 0, 1],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
    },
    Face {
        normal: [0, 0, -1],
        corners: [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
    },
];
