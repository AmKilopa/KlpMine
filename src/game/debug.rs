use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::renderer::RenderAdapterInfo,
};
use sysinfo::{ProcessesToUpdate, System};

use crate::game::{
    camera::{PlayerCamera, player_view},
    events::GameplayStats,
    health::PlayerHealth,
    resources::ResourceManager,
};

pub struct DebugPlugin;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct DebugText;

#[derive(Resource)]
struct DebugState {
    visible: bool,
    system: System,
    timer: Timer,
    snapshot: SystemSnapshot,
}

#[derive(Default)]
struct SystemSnapshot {
    system_cpu: f32,
    game_cpu: f32,
    system_memory_total: u64,
    system_memory_used: u64,
    game_memory: u64,
    logical_cores: usize,
    cpu_name: String,
}

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .insert_resource(DebugState::new())
            .add_systems(Startup, spawn_debug_panel)
            .add_systems(
                Update,
                (toggle_debug_panel, refresh_debug_state, update_debug_text),
            );
    }
}

impl DebugState {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_cpu_all();
        system.refresh_memory();
        system.refresh_processes(ProcessesToUpdate::All, true);

        Self {
            visible: false,
            system,
            timer: Timer::from_seconds(0.35, TimerMode::Repeating),
            snapshot: SystemSnapshot::default(),
        }
    }
}

fn spawn_debug_panel(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(12),
                padding: UiRect::all(px(10)),
                max_width: px(760),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.64)),
            Visibility::Hidden,
            GlobalZIndex(i32::MAX),
            DebugPanel,
        ))
        .with_child((
            Text::new(""),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.95, 0.9)),
            DebugText,
        ));
}

fn toggle_debug_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DebugState>,
    mut panels: Query<&mut Visibility, With<DebugPanel>>,
) {
    if !keys.just_pressed(KeyCode::F3) {
        return;
    }

    state.visible = !state.visible;

    for mut visibility in &mut panels {
        *visibility = if state.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn refresh_debug_state(time: Res<Time>, mut state: ResMut<DebugState>) {
    state.timer.tick(time.delta());

    if !state.visible || !state.timer.just_finished() {
        return;
    }

    state.system.refresh_cpu_usage();
    state.system.refresh_memory();

    let current_pid = sysinfo::get_current_pid().ok();
    if let Some(pid) = current_pid {
        state
            .system
            .refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }

    let process = current_pid.and_then(|pid| state.system.process(pid));
    let logical_cores = state.system.cpus().len();
    let cpu_name = state
        .system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    state.snapshot = SystemSnapshot {
        system_cpu: state.system.global_cpu_usage(),
        game_cpu: game_cpu_percent(
            process
                .map(|process| process.cpu_usage())
                .unwrap_or_default(),
            logical_cores,
        ),
        system_memory_total: state.system.total_memory(),
        system_memory_used: state.system.used_memory(),
        game_memory: process.map(|process| process.memory()).unwrap_or_default(),
        logical_cores,
        cpu_name,
    };
}

fn update_debug_text(
    state: Res<DebugState>,
    diagnostics: Res<DiagnosticsStore>,
    adapter: Option<Res<RenderAdapterInfo>>,
    resources: Option<Res<ResourceManager>>,
    gameplay_stats: Option<Res<GameplayStats>>,
    health: Option<Res<PlayerHealth>>,
    cameras: Query<(&Transform, &super::camera::PlayerController), With<PlayerCamera>>,
    entities: Query<Entity>,
    mut texts: Query<&mut Text, With<DebugText>>,
) {
    if !state.visible {
        return;
    }

    let Some(view) = player_view(&cameras) else {
        return;
    };
    let Ok(mut text) = texts.single_mut() else {
        return;
    };

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed())
        .unwrap_or_default();
    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|value| value.smoothed())
        .unwrap_or_default();
    let gpu = adapter
        .as_ref()
        .map(|adapter| adapter.0.name.clone())
        .unwrap_or_else(|| "loading".to_string());
    let hot_reload = resources
        .as_ref()
        .map(|resources| resources.hot_reload)
        .unwrap_or(false);
    let atlas_loaded = resources
        .as_ref()
        .map(|resources| resources.block_atlas.is_strong())
        .unwrap_or(false);
    let stats = gameplay_stats.as_deref();
    let broken_blocks = stats.map(|stats| stats.broken_blocks).unwrap_or_default();
    let placed_blocks = stats.map(|stats| stats.placed_blocks).unwrap_or_default();
    let picked_items = stats.map(|stats| stats.picked_items).unwrap_or_default();
    let last_mass = stats.map(|stats| stats.last_block_mass).unwrap_or_default();
    let hp = health
        .as_ref()
        .map(|health| health.current)
        .unwrap_or_default();

    text.0 = format!(
        "KlpMine Debug\n\
         FPS: {:.0} | Frame: {:.2} ms\n\
         XYZ: {:.3} / {:.3} / {:.3}\n\
         Block: {} {} {}\n\
         Facing: {} | Yaw: {:.1} | Pitch: {:.1}\n\
         Game CPU: {:.1}% | System CPU: {:.1}%\n\
         Game RAM: {} MB | System RAM Used: {} / {} MB\n\
         Entities: {} | Hot Reload: {} | Atlas: {}\n\
         HP: {:.1}\n\
         Blocks: broken {} / placed {} | Picked: {} | Last Mass: {:.2}\n\
         CPU: {} | Cores: {}\n\
         GPU: {}",
        fps,
        frame_time,
        view.position.x,
        view.position.y,
        view.position.z,
        view.position.x.floor() as i32,
        view.position.y.floor() as i32,
        view.position.z.floor() as i32,
        compass(view.yaw),
        view.yaw.to_degrees(),
        view.pitch.to_degrees(),
        state.snapshot.game_cpu,
        state.snapshot.system_cpu,
        bytes_to_mb(state.snapshot.game_memory),
        bytes_to_mb(state.snapshot.system_memory_used),
        bytes_to_mb(state.snapshot.system_memory_total),
        entities.iter().count(),
        if hot_reload { "on" } else { "off" },
        if atlas_loaded { "loaded" } else { "pending" },
        hp,
        broken_blocks,
        placed_blocks,
        picked_items,
        last_mass,
        state.snapshot.cpu_name,
        state.snapshot.logical_cores,
        gpu
    );
}

fn compass(yaw: f32) -> &'static str {
    let degrees = yaw.to_degrees().rem_euclid(360.0);

    match degrees {
        value if !(22.5..337.5).contains(&value) => "north",
        value if value < 67.5 => "north-west",
        value if value < 112.5 => "west",
        value if value < 157.5 => "south-west",
        value if value < 202.5 => "south",
        value if value < 247.5 => "south-east",
        value if value < 292.5 => "east",
        _ => "north-east",
    }
}

fn game_cpu_percent(raw_cpu: f32, logical_cores: usize) -> f32 {
    if logical_cores == 0 {
        return raw_cpu;
    }

    (raw_cpu / logical_cores as f32).clamp(0.0, 100.0)
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}
