use bevy::prelude::*;

#[derive(Resource)]
pub struct BlockMaterials {
    pub terrain: Handle<StandardMaterial>,
    pub held_terrain: Handle<StandardMaterial>,
    pub particle: Handle<StandardMaterial>,
}

pub fn setup_materials(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(BlockMaterials {
        terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("textures/block_atlas.png")),
            perceptual_roughness: 0.9,
            ..default()
        }),
        held_terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("textures/block_atlas.png")),
            perceptual_roughness: 0.9,
            unlit: true,
            ..default()
        }),
        particle: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.28, 0.14),
            perceptual_roughness: 0.95,
            ..default()
        }),
    });
}
