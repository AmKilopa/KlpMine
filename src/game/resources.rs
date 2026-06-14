use bevy::prelude::*;

pub struct ResourceManagerPlugin;

#[derive(Resource)]
pub struct ResourceManager {
    pub block_atlas: Handle<Image>,
    pub hot_reload: bool,
}

impl Plugin for ResourceManagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_resource_manager);
    }
}

fn setup_resource_manager(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ResourceManager {
        block_atlas: asset_server.load("textures/block_atlas.png"),
        hot_reload: asset_server.watching_for_changes(),
    });
}
