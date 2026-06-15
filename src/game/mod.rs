use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

pub mod audio;
pub mod camera;
pub mod chat;
pub mod debug;
pub mod events;
pub mod hand;
pub mod health;
pub mod inventory;
pub mod resources;
pub mod settings;
pub mod sky;
pub mod world;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

        app.add_plugins((
            resources::ResourceManagerPlugin,
            events::GameEventsPlugin,
            settings::SettingsPlugin,
            chat::ChatPlugin,
            inventory::InventoryPlugin,
            health::HealthPlugin,
            camera::CameraPlugin,
            world::WorldPlugin,
            sky::SkyPlugin,
            hand::HandPlugin,
            debug::DebugPlugin,
        ));
    }
}
