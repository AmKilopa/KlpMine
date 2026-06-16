use std::path::Path;

use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};

use crate::game::settings::GameSettings;

pub struct GameAudioPlugin;

#[derive(Component)]
struct BackgroundMusic;

#[derive(Component)]
struct AmbienceLoop;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_background_audio)
            .add_systems(Update, apply_audio_settings);
    }
}

pub fn optional_sound(
    asset_server: &AssetServer,
    path: &'static str,
) -> Option<Handle<AudioSource>> {
    Path::new("assets")
        .join(path)
        .exists()
        .then(|| asset_server.load(path))
}

pub fn effect_playback(settings: &GameSettings) -> PlaybackSettings {
    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.effects_volume))
}

fn spawn_background_audio(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    settings: Res<GameSettings>,
) {
    if let Some(music) = optional_sound(&asset_server, "music/background.ogg") {
        commands.spawn((
            AudioPlayer::new(music),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(settings.music_volume)),
            BackgroundMusic,
        ));
    }

    if let Some(ambience) = optional_sound(&asset_server, "ambience/forest.ogg") {
        commands.spawn((
            AudioPlayer::new(ambience),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(settings.ambience_volume)),
            AmbienceLoop,
        ));
    }
}

fn apply_audio_settings(
    settings: Res<GameSettings>,
    mut global_volume: ResMut<GlobalVolume>,
    mut music: Query<&mut AudioSink, With<BackgroundMusic>>,
    mut ambience: Query<&mut AudioSink, (With<AmbienceLoop>, Without<BackgroundMusic>)>,
) {
    if !settings.is_changed() {
        return;
    }

    *global_volume = GlobalVolume::new(Volume::Linear(settings.master_volume));

    for mut sink in &mut music {
        sink.set_volume(Volume::Linear(settings.music_volume));
    }

    for mut sink in &mut ambience {
        sink.set_volume(Volume::Linear(settings.ambience_volume));
    }
}
