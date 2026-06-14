use bevy::prelude::*;

pub mod audio;
pub mod camera;
pub mod debug;
pub mod events;
pub mod hand;
pub mod health;
pub mod inventory;
pub mod resources;
pub mod settings;
pub mod world;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            resources::ResourceManagerPlugin,
            events::GameEventsPlugin,
            settings::SettingsPlugin,
            inventory::InventoryPlugin,
            health::HealthPlugin,
            camera::CameraPlugin,
            world::WorldPlugin,
            hand::HandPlugin,
            debug::DebugPlugin,
        ));
    }
}
