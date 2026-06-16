use bevy::prelude::*;

const PHYSICS_ROUGHNESS: f32 = 0.95;

#[derive(Resource)]
pub struct BlockMaterials {
    pub terrain: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub held_terrain: Handle<StandardMaterial>,
    pub particle: Handle<StandardMaterial>,
    pub leaf_particle: Handle<StandardMaterial>,
    pub debug_marker: Handle<StandardMaterial>,
    pub shadow: Handle<StandardMaterial>,
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
            perceptual_roughness: 0.96,
            reflectance: 0.18,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(0.36, 0.64, 0.92, 0.68),
            base_color_texture: Some(atlas.clone()),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.18,
            reflectance: 0.52,
            specular_tint: Color::srgb(0.58, 0.78, 1.0),
            ..default()
        }),
        held_terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(atlas),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.96,
            reflectance: 0.18,
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
        shadow: materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.34),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            ..default()
        }),
        debug_marker: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.04, 0.02),
            emissive: LinearRgba::rgb(4.0, 0.05, 0.02),
            perceptual_roughness: 0.35,
            unlit: true,
            ..default()
        }),
    });
}
