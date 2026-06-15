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
    settings::GameSettings,
    sky::LightingState,
    world::Chunk,
};

pub struct DebugPlugin;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct DebugText;

#[derive(Resource)]
struct DebugState {
    visible: bool,
    page: DebugPage,
    system: System,
    timer: Timer,
    snapshot: SystemSnapshot,
}

#[derive(Resource, Default)]
pub struct PhysicsDebug {
    pub enabled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DebugPage {
    Main,
    Light,
    Performance,
    Gameplay,
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
            .insert_resource(PhysicsDebug::default())
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
            page: DebugPage::Main,
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
    mut physics_debug: ResMut<PhysicsDebug>,
    mut panels: Query<&mut Visibility, With<DebugPanel>>,
) {
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let debug_shift = state.visible && shift_held;

    if debug_shift {
        if keys.just_pressed(KeyCode::KeyL) {
            state.page = DebugPage::Light;
            return;
        }
        if keys.just_pressed(KeyCode::KeyP) {
            state.page = DebugPage::Performance;
            return;
        }
        if keys.just_pressed(KeyCode::KeyG) {
            state.page = DebugPage::Gameplay;
            return;
        }
        if keys.just_pressed(KeyCode::KeyM) {
            state.page = DebugPage::Main;
            return;
        }
        if keys.just_pressed(KeyCode::KeyH) {
            physics_debug.enabled = !physics_debug.enabled;
            return;
        }
    }

    if keys.pressed(KeyCode::F3) {
        if keys.just_pressed(KeyCode::KeyH) {
            state.visible = true;
            physics_debug.enabled = !physics_debug.enabled;
            set_debug_visibility(true, &mut panels);
            return;
        }
        if keys.just_pressed(KeyCode::KeyL) {
            state.visible = true;
            state.page = DebugPage::Light;
            set_debug_visibility(true, &mut panels);
            return;
        }
        if keys.just_pressed(KeyCode::KeyP) {
            state.visible = true;
            state.page = DebugPage::Performance;
            set_debug_visibility(true, &mut panels);
            return;
        }
        if keys.just_pressed(KeyCode::KeyG) {
            state.visible = true;
            state.page = DebugPage::Gameplay;
            set_debug_visibility(true, &mut panels);
            return;
        }
    }

    if !keys.just_pressed(KeyCode::F3) {
        return;
    }

    state.visible = !state.visible;
    state.page = DebugPage::Main;
    set_debug_visibility(state.visible, &mut panels);
}

fn set_debug_visibility(visible: bool, panels: &mut Query<&mut Visibility, With<DebugPanel>>) {
    for mut visibility in panels {
        *visibility = if visible {
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
            process.map(|p| p.cpu_usage()).unwrap_or_default(),
            logical_cores,
        ),
        system_memory_total: state.system.total_memory(),
        system_memory_used: state.system.used_memory(),
        game_memory: process.map(|p| p.memory()).unwrap_or_default(),
        logical_cores,
        cpu_name,
    };
}

fn update_debug_text(
    state: Res<DebugState>,
    diagnostics: Res<DiagnosticsStore>,
    adapter: Option<Res<RenderAdapterInfo>>,
    resources: Option<Res<ResourceManager>>,
    settings: Option<Res<GameSettings>>,
    lighting: Option<Res<LightingState>>,
    gameplay_stats: Option<Res<GameplayStats>>,
    health: Option<Res<PlayerHealth>>,
    cameras: Query<(&Transform, &super::camera::PlayerController), With<PlayerCamera>>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
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
        .and_then(|d| d.smoothed())
        .unwrap_or_default();
    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or_default();
    let gpu = adapter
        .as_ref()
        .map(|a| a.0.name.clone())
        .unwrap_or_else(|| "loading".to_string());
    let hot_reload = resources.as_ref().map(|r| r.hot_reload).unwrap_or(false);
    let atlas_loaded = resources
        .as_ref()
        .map(|r| r.block_atlas.is_strong())
        .unwrap_or(false);
    let stats = gameplay_stats.as_deref();
    let broken_blocks = stats.map(|s| s.broken_blocks).unwrap_or_default();
    let placed_blocks = stats.map(|s| s.placed_blocks).unwrap_or_default();
    let picked_items = stats.map(|s| s.picked_items).unwrap_or_default();
    let last_mass = stats.map(|s| s.last_block_mass).unwrap_or_default();
    let hp = health.as_ref().map(|h| h.current).unwrap_or_default();
    let render_distance = settings
        .as_ref()
        .map(|s| s.render_distance)
        .unwrap_or_default();
    let time_label = lighting.as_ref().map(|l| l.label).unwrap_or("loading");
    let sky_light = lighting.as_ref().map(|l| l.sky_light).unwrap_or_default();
    let day_factor = lighting.as_ref().map(|l| l.day_factor).unwrap_or_default();
    let sun_angle = lighting.as_ref().map(|l| l.sun_angle).unwrap_or_default();
    let clock_minutes = lighting
        .as_ref()
        .map(|l| l.clock_minutes)
        .unwrap_or_default();
    let clock = format!("{:02}:{:02}", clock_minutes / 60, clock_minutes % 60);
    let current_block = block_at_debug(view.position.floor().as_ivec3(), &chunks);
    let block_light = current_block
        .emitted_light()
        .max(lighting.as_ref().map(|l| l.block_light).unwrap_or_default());

    text.0 = match state.page {
        DebugPage::Main => format!(
            "KlpMine Debug\n\
             F3 main | Shift+L light | Shift+P performance | Shift+G gameplay\n\
             Shift+H physics hitboxes\n\
             FPS: {:.0} | Frame: {:.2} ms\n\
             XYZ: {:.3} / {:.3} / {:.3}\n\
             Block: {} {} {}\n\
             Facing: {} | Yaw: {:.1} | Pitch: {:.1}\n\
             HP: {:.1}\n\
             Chunks: loaded {} | distance {}\n\
             Light: sky {} / block {} | {} {}",
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
            hp,
            chunks.iter().count(),
            render_distance,
            sky_light,
            block_light,
            time_label,
            clock
        ),
        DebugPage::Light => format!(
            "KlpMine Light Debug\n\
             F3 main | Shift+M main | Shift+P performance | Shift+G gameplay\n\
             Sky light: {} / 15\n\
             Block light: {} / 15\n\
             Block emits: {} / 15\n\
             Time: {} | Phase: {} | Day factor: {:.3}\n\
             Sun angle: {:.1}\n\
             Sun brightness: {:.0}\n\
             Ambient brightness: {:.0}",
            sky_light,
            block_light,
            current_block.emitted_light(),
            clock,
            time_label,
            day_factor,
            sun_angle,
            if day_factor > 0.08 {
                3_000.0 + day_factor * 28_000.0
            } else {
                2_400.0
            },
            210.0 + day_factor * 360.0
        ),
        DebugPage::Performance => format!(
            "KlpMine Performance Debug\n\
             F3 main | Shift+M main | Shift+L light | Shift+G gameplay\n\
             FPS: {:.0}\n\
             Frame: {:.2} ms\n\
             Game CPU: {:.1}%\n\
             System CPU: {:.1}%\n\
             Game RAM: {} MB\n\
             System RAM: {} / {} MB\n\
             Entities: {}\n\
             Chunks: loaded {} | distance {}\n\
             GPU: {}",
            fps,
            frame_time,
            state.snapshot.game_cpu,
            state.snapshot.system_cpu,
            bytes_to_mb(state.snapshot.game_memory),
            bytes_to_mb(state.snapshot.system_memory_used),
            bytes_to_mb(state.snapshot.system_memory_total),
            entities.iter().count(),
            chunks.iter().count(),
            render_distance,
            gpu
        ),
        DebugPage::Gameplay => format!(
            "KlpMine Gameplay Debug\n\
             F3 main | Shift+M main | Shift+L light | Shift+P performance\n\
             HP: {:.1}\n\
             Blocks: broken {} / placed {}\n\
             Picked items: {}\n\
             Last mass: {:.2}\n\
             Hot reload: {}\n\
             Atlas: {}\n\
             CPU: {} | Cores: {}",
            hp,
            broken_blocks,
            placed_blocks,
            picked_items,
            last_mass,
            if hot_reload { "on" } else { "off" },
            if atlas_loaded { "loaded" } else { "pending" },
            state.snapshot.cpu_name,
            state.snapshot.logical_cores
        ),
    };
}

fn block_at_debug(
    world_pos: IVec3,
    chunks: &Query<(&Chunk, &GlobalTransform)>,
) -> crate::game::world::Block {
    use crate::game::world::Chunk as C;
    for (chunk, transform) in chunks {
        let local = world_pos - transform.translation().floor().as_ivec3();
        if C::contains(local) {
            return chunk.get(local.x, local.y, local.z);
        }
    }
    crate::game::world::Block::Air
}

fn compass(yaw: f32) -> &'static str {
    let degrees = yaw.to_degrees().rem_euclid(360.0);
    match degrees {
        v if !(22.5..337.5).contains(&v) => "north",
        v if v < 67.5 => "north-west",
        v if v < 112.5 => "west",
        v if v < 157.5 => "south-west",
        v if v < 202.5 => "south",
        v if v < 247.5 => "south-east",
        v if v < 292.5 => "east",
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
