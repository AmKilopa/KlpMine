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

const ATLAS_WIDTH: f32 = 238.0;
const ATLAS_HEIGHT: f32 = 34.0;
const CELL_SIZE: f32 = 34.0;
const UV_INSET: f32 = 0.5;

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
    let mut builder = MeshBuilder::default();

    for y in 0..CHUNK_HEIGHT as i32 {
        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                let block = chunk.get(x, y, z);
                if !block.is_solid() {
                    continue;
                }
                let local = IVec3::new(x, y, z);
                for face in FACES {
                    if !block_at(local + IVec3::from_array(face.normal)).is_solid() {
                        builder.add_face(block, local.as_vec3(), face);
                    }
                }
            }
        }
    }

    (!builder.is_empty()).then(|| builder.build())
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

fn face_light(normal: [i32; 3]) -> f32 {
    match normal {
        [0, 1, 0] => 1.0,
        [0, -1, 0] => 0.45,
        [1, 0, 0] => 0.72,
        [-1, 0, 0] => 0.62,
        [0, 0, 1] => 0.82,
        [0, 0, -1] => 0.55,
        _ => 0.7,
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
