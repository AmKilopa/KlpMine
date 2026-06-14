use bevy::prelude::*;

use crate::game::GamePlugin;

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.95)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KlpMine".to_string(),
                resolution: (1280, 720).into(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GamePlugin)
        .run();
}
