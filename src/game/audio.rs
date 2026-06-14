use std::path::Path;

use bevy::prelude::*;

pub fn optional_sound(
    asset_server: &AssetServer,
    path: &'static str,
) -> Option<Handle<AudioSource>> {
    Path::new("assets")
        .join(path)
        .exists()
        .then(|| asset_server.load(path))
}
