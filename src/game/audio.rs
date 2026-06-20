use std::path::PathBuf;

use bevy::{
    audio::{AudioSink, AudioSinkPlayback, Volume},
    prelude::*,
};

use crate::game::settings::GameSettings;
use crate::game::sky::LightingState;

pub struct GameAudioPlugin;

#[derive(Component)]
struct BackgroundMusic;

#[derive(Component)]
struct AmbienceDay;

#[derive(Component)]
struct AmbienceNight;

#[derive(Component)]
struct FadeVolume {
    current: f32,
    target: f32,
}

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_background_audio)
            .add_systems(Update, (apply_audio_settings, fade_audio_volumes, update_ambience_crossfade));
    }
}

pub fn optional_sound(
    asset_server: &AssetServer,
    path: &'static str,
) -> Option<Handle<AudioSource>> {
    let exists = PathBuf::from("assets").join(path).exists()
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(path)
            .exists();
    if exists {
        Some(asset_server.load(path))
    } else {
        None
    }
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
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
            BackgroundMusic,
            FadeVolume {
                current: 0.0,
                target: settings.music_volume,
            },
        ));
    }

    if let Some(day) = optional_sound(&asset_server, "ambience/ambience_day.ogg") {
        commands.spawn((
            AudioPlayer::new(day),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
            AmbienceDay,
            FadeVolume {
                current: 0.0,
                target: settings.ambience_volume,
            },
        ));
    }

    if let Some(night) = optional_sound(&asset_server, "ambience/ambience_night.ogg") {
        commands.spawn((
            AudioPlayer::new(night),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
            AmbienceNight,
            FadeVolume {
                current: 0.0,
                target: 0.0,
            },
        ));
    }
}

fn apply_audio_settings(
    settings: Res<GameSettings>,
    mut global_volume: ResMut<GlobalVolume>,
    mut music: Query<&mut FadeVolume, (With<BackgroundMusic>, Without<AmbienceDay>)>,
    mut ambience: Query<&mut FadeVolume, (With<AmbienceDay>, Without<BackgroundMusic>)>,
) {
    if !settings.is_changed() {
        return;
    }

    *global_volume = GlobalVolume::new(Volume::Linear(settings.master_volume));

    for mut fade in &mut music {
        fade.target = settings.music_volume;
    }

    for mut fade in &mut ambience {
        fade.target = settings.ambience_volume;
    }
}

fn fade_audio_volumes(time: Res<Time>, mut sinks: Query<(&mut FadeVolume, &mut AudioSink)>) {
    let speed = time.delta_secs() * 2.0;
    for (mut fade, mut sink) in &mut sinks {
        if (fade.current - fade.target).abs() > 0.001 {
            fade.current += (fade.target - fade.current).clamp(-speed, speed);
            sink.set_volume(Volume::Linear(fade.current));
        }
    }
}

fn update_ambience_crossfade(
    lighting: Res<LightingState>,
    mut day: Query<&mut FadeVolume, (With<AmbienceDay>, Without<AmbienceNight>)>,
    mut night: Query<&mut FadeVolume, (With<AmbienceNight>, Without<AmbienceDay>)>,
    settings: Res<GameSettings>,
) {
    let day_factor = lighting.day_factor;

    for mut fade in &mut day {
        fade.target = day_factor * settings.ambience_volume;
    }

    for mut fade in &mut night {
        fade.target = (1.0 - day_factor) * settings.ambience_volume;
    }
}
