use bevy::prelude::*;

pub mod camera;
pub mod debug;
pub mod world;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((camera::CameraPlugin, world::WorldPlugin, debug::DebugPlugin));
    }
}
