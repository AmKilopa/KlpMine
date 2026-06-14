use bevy::{asset::AssetPlugin, prelude::*};

use crate::game::GamePlugin;

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.95)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.58, 0.68, 0.82),
            brightness: 420.0,
            ..default()
        })
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    watch_for_changes_override: Some(true),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "KlpMine".to_string(),
                        resolution: (1280, 720).into(),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(GamePlugin)
        .run();
}
