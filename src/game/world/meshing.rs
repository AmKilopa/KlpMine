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

pub fn build_chunk_mesh(chunk: &Chunk) -> Option<Mesh> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
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
                    let neighbor =
                        chunk.get(x + face.normal[0], y + face.normal[1], z + face.normal[2]);
                    if neighbor.is_solid() {
                        continue;
                    }

                    add_face(
                        block,
                        Vec3::new(x as f32, y as f32, z as f32),
                        face,
                        &mut positions,
                        &mut normals,
                        &mut colors,
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
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
    colors: &mut Vec<[f32; 4]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    if block == Block::Grass && face.normal[1] == 0 {
        add_grass_side_face(origin, face, positions, normals, colors, uvs, indices);
        return;
    }

    let base = positions.len() as u32;
    let face_colors = block_face_colors(block, face.normal);

    for (index, corner) in face.corners.iter().enumerate() {
        let position = origin + Vec3::new(corner[0], corner[1], corner[2]);
        positions.push([position.x, position.y, position.z]);
        normals.push([
            face.normal[0] as f32,
            face.normal[1] as f32,
            face.normal[2] as f32,
        ]);
        colors.push(face_colors[index]);
    }

    uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn add_grass_side_face(
    origin: Vec3,
    face: Face,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    let dirt = [0.42, 0.27, 0.14, 1.0];
    let grass = [0.18, 0.42, 0.12, 1.0];
    let split = 0.72;

    let bottom_left = face.corners[0];
    let bottom_right = face.corners[1];
    let top_right = face.corners[2];
    let top_left = face.corners[3];
    let mid_left = lerp_corner(bottom_left, top_left, split);
    let mid_right = lerp_corner(bottom_right, top_right, split);

    add_colored_quad(
        [bottom_left, bottom_right, mid_right, mid_left],
        origin,
        face.normal,
        dirt,
        positions,
        normals,
        colors,
        uvs,
        indices,
    );
    add_colored_quad(
        [mid_left, mid_right, top_right, top_left],
        origin,
        face.normal,
        grass,
        positions,
        normals,
        colors,
        uvs,
        indices,
    );
}

fn add_colored_quad(
    corners: [[f32; 3]; 4],
    origin: Vec3,
    normal: [i32; 3],
    color: [f32; 4],
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    let base = positions.len() as u32;

    for corner in corners {
        let position = origin + Vec3::new(corner[0], corner[1], corner[2]);
        positions.push([position.x, position.y, position.z]);
        normals.push([normal[0] as f32, normal[1] as f32, normal[2] as f32]);
        colors.push(color);
    }

    uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn lerp_corner(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn block_face_colors(block: Block, normal: [i32; 3]) -> [[f32; 4]; 4] {
    let dirt = [0.42, 0.27, 0.14, 1.0];
    let stone = [0.48, 0.48, 0.52, 1.0];
    let grass_top = [0.14, 0.36, 0.10, 1.0];
    let grass_side = [0.18, 0.42, 0.12, 1.0];

    match block {
        Block::Air => [[1.0, 1.0, 1.0, 1.0]; 4],
        Block::Dirt => [dirt; 4],
        Block::Stone => [stone; 4],
        Block::Grass if normal == [0, 1, 0] => [grass_top; 4],
        Block::Grass if normal == [0, -1, 0] => [dirt; 4],
        Block::Grass => [dirt, dirt, grass_side, grass_side],
    }
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
