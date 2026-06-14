use bevy::prelude::*;

#[derive(Resource)]
pub struct BlockMaterials {
    pub terrain: Handle<StandardMaterial>,
}

pub fn setup_materials(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(BlockMaterials {
        terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            ..default()
        }),
    });
}
