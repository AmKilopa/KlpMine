use bevy::prelude::*;

const PHYSICS_ROUGHNESS: f32 = 0.95;

#[derive(Resource)]
pub struct BlockMaterials {
    pub terrain: Handle<StandardMaterial>,
    pub held_terrain: Handle<StandardMaterial>,
    pub particle: Handle<StandardMaterial>,
    pub leaf_particle: Handle<StandardMaterial>,
    pub log_physics: Handle<StandardMaterial>,
}

pub fn setup_materials(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let atlas = asset_server.load("textures/block_atlas.png");

    commands.insert_resource(BlockMaterials {
        terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(atlas.clone()),
            perceptual_roughness: 0.9,
            ..default()
        }),
        held_terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(atlas),
            perceptual_roughness: 0.9,
            unlit: true,
            ..default()
        }),
        particle: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.28, 0.14),
            perceptual_roughness: PHYSICS_ROUGHNESS,
            ..default()
        }),
        leaf_particle: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.52, 0.24),
            perceptual_roughness: PHYSICS_ROUGHNESS,
            ..default()
        }),
        log_physics: materials.add(StandardMaterial {
            base_color: Color::srgb(0.44, 0.25, 0.12),
            perceptual_roughness: PHYSICS_ROUGHNESS,
            ..default()
        }),
    });
}
