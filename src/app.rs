use std::path::PathBuf;

use bevy::{asset::AssetPlugin, prelude::*};

use crate::game::GamePlugin;

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.5, 0.7, 0.95)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.62, 0.68, 0.84),
            brightness: 760.0,
            ..default()
        })
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: asset_path(),
                    watch_for_changes_override: Some(false),
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

fn asset_path() -> String {
    let current_assets = std::env::current_dir()
        .ok()
        .map(|path| path.join("assets"))
        .filter(|path| path.exists());
    let path =
        current_assets.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));

    path.to_string_lossy().replace('\\', "/")
}
