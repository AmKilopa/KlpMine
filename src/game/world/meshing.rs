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

pub fn build_chunk_mesh_with_neighbors(
    chunk: &Chunk,
    block_at: impl Fn(IVec3) -> Block,
) -> Option<Mesh> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for y in 0..CHUNK_HEIGHT as i32 {
        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                let block = chunk.get(x, y, z);
                if !block.is_solid() {
                    continue;
                }

                for face in FACES {
                    let local = IVec3::new(x, y, z);
                    let neighbor = block_at(local + IVec3::from_array(face.normal));
                    if neighbor.is_solid() {
                        continue;
                    }

                    add_face(
                        block,
                        local.as_vec3(),
                        face,
                        &mut positions,
                        &mut normals,
                        &mut uvs,
                        &mut indices,
                    );
                }
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

fn add_face(
    block: Block,
    origin: Vec3,
    face: Face,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    let base = positions.len() as u32;
    let face_uvs = tile_uvs(face_tile(block, face.normal));

    for corner in face.corners {
        let position = origin + Vec3::new(corner[0], corner[1], corner[2]);
        positions.push([position.x, position.y, position.z]);
        normals.push([
            face.normal[0] as f32,
            face.normal[1] as f32,
            face.normal[2] as f32,
        ]);
    }

    uvs.extend_from_slice(&face_uvs);
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn face_tile(block: Block, normal: [i32; 3]) -> AtlasTile {
    match block {
        Block::Grass if normal == [0, 1, 0] => AtlasTile::GrassTop,
        Block::Grass if normal == [0, -1, 0] => AtlasTile::Dirt,
        Block::Grass => AtlasTile::GrassSide,
        Block::Air | Block::Dirt => AtlasTile::Dirt,
    }
}

fn tile_uvs(tile: AtlasTile) -> [[f32; 2]; 4] {
    let index = tile as u32;
    let atlas_width = 102.0;
    let atlas_height = 34.0;
    let cell_size = 34.0;
    let inset = 0.5;
    let u0 = (index as f32 * cell_size + 1.0 + inset) / atlas_width;
    let u1 = (index as f32 * cell_size + 33.0 - inset) / atlas_width;
    let v0 = (1.0 + inset) / atlas_height;
    let v1 = (33.0 - inset) / atlas_height;

    [[u0, v1], [u1, v1], [u1, v0], [u0, v0]]
}

#[derive(Clone, Copy)]
enum AtlasTile {
    Dirt = 0,
    GrassTop = 1,
    GrassSide = 2,
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
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
    },
    Face {
        normal: [-1, 0, 0],
        corners: [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
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
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ],
    },
    Face {
        normal: [0, 0, -1],
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
    },
];
