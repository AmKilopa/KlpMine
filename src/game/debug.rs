use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::system::SystemParam,
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
    world::{Chunk, WaterSimulation, WorldSeed},
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

type DebugCameraQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, &'static super::camera::PlayerController),
    With<PlayerCamera>,
>;

#[derive(SystemParam)]
struct DebugInputs<'w, 's> {
    diagnostics: Res<'w, DiagnosticsStore>,
    adapter: Option<Res<'w, RenderAdapterInfo>>,
    resources: Option<Res<'w, ResourceManager>>,
    images: Res<'w, Assets<Image>>,
    settings: Option<Res<'w, GameSettings>>,
    lighting: Option<Res<'w, LightingState>>,
    seed: Option<Res<'w, WorldSeed>>,
    water: Option<Res<'w, WaterSimulation>>,
    gameplay_stats: Option<Res<'w, GameplayStats>>,
    health: Option<Res<'w, PlayerHealth>>,
    cameras: DebugCameraQuery<'w, 's>,
    chunks: Query<'w, 's, (&'static Chunk, &'static GlobalTransform)>,
    entities: Query<'w, 's, Entity>,
    texts: Query<'w, 's, &'static mut Text, With<DebugText>>,
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

fn update_debug_text(state: Res<DebugState>, mut inputs: DebugInputs) {
    if !state.visible {
        return;
    }

    let Some(view) = player_view(&inputs.cameras) else {
        return;
    };
    let Ok(mut text) = inputs.texts.single_mut() else {
        return;
    };

    let fps = inputs
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or_default();
    let frame_time = inputs
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or_default();
    let gpu = inputs
        .adapter
        .as_ref()
        .map(|a| a.0.name.clone())
        .unwrap_or_else(|| "loading".to_string());
    let hot_reload = inputs
        .resources
        .as_ref()
        .map(|r| r.hot_reload)
        .unwrap_or(false);
    let atlas_loaded = inputs
        .resources
        .as_ref()
        .map(|r| inputs.images.get(&r.block_atlas).is_some())
        .unwrap_or(false);
    let stats = inputs.gameplay_stats.as_deref();
    let broken_blocks = stats.map(|s| s.broken_blocks).unwrap_or_default();
    let placed_blocks = stats.map(|s| s.placed_blocks).unwrap_or_default();
    let picked_items = stats.map(|s| s.picked_items).unwrap_or_default();
    let last_mass = stats.map(|s| s.last_block_mass).unwrap_or_default();
    let hp = inputs
        .health
        .as_ref()
        .map(|h| h.current)
        .unwrap_or_default();
    let render_distance = inputs
        .settings
        .as_ref()
        .map(|s| s.render_distance)
        .unwrap_or_default();
    let graphics_state = inputs
        .settings
        .as_ref()
        .map(|s| {
            format!(
                "Bloom {} | Motion {} | Color {} | Fog {} | Soft shadows {}",
                on_off(s.bloom),
                on_off(s.motion_blur),
                on_off(s.color_grading),
                on_off(s.fog),
                on_off(s.shadows)
            )
        })
        .unwrap_or_else(|| "loading".to_string());
    let seed_value = inputs.seed.as_ref().map(|s| s.value).unwrap_or_default();
    let water_stats = inputs.water.as_ref().map(|water| water.debug_stats());
    let time_label = inputs
        .lighting
        .as_ref()
        .map(|l| l.label)
        .unwrap_or("loading");
    let sky_light = inputs
        .lighting
        .as_ref()
        .map(|l| l.sky_light)
        .unwrap_or_default();
    let day_factor = inputs
        .lighting
        .as_ref()
        .map(|l| l.day_factor)
        .unwrap_or_default();
    let sun_angle = inputs
        .lighting
        .as_ref()
        .map(|l| l.sun_angle)
        .unwrap_or_default();
    let clock_minutes = inputs
        .lighting
        .as_ref()
        .map(|l| l.clock_minutes)
        .unwrap_or_default();
    let clock = format!("{:02}:{:02}", clock_minutes / 60, clock_minutes % 60);
    let current_block = block_at_debug(view.position.floor().as_ivec3(), &inputs.chunks);
    let block_light = current_block.emitted_light().max(
        inputs
            .lighting
            .as_ref()
            .map(|l| l.block_light)
            .unwrap_or_default(),
    );

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
             Seed: {}\n\
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
            inputs.chunks.iter().count(),
            render_distance,
            seed_value,
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
             Ambient brightness: {:.0}\n\
             Effects: HDR bloom color-grade motion-blur | Soft shadows: {}",
            sky_light,
            block_light,
            current_block.emitted_light(),
            clock,
            time_label,
            day_factor,
            sun_angle,
            if day_factor > 0.08 {
                2_400.0 + day_factor * 6_800.0
            } else {
                560.0
            },
            520.0 + day_factor * 760.0,
            inputs
                .settings
                .as_ref()
                .map(|s| on_off(s.shadows))
                .unwrap_or("loading")
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
              GPU: {}\n\
              Graphics: HDR on | {}",
            fps,
            frame_time,
            state.snapshot.game_cpu,
            state.snapshot.system_cpu,
            bytes_to_mb(state.snapshot.game_memory),
            bytes_to_mb(state.snapshot.system_memory_used),
            bytes_to_mb(state.snapshot.system_memory_total),
            inputs.entities.iter().count(),
            inputs.chunks.iter().count(),
            render_distance,
            gpu,
            graphics_state
        ),
        DebugPage::Gameplay => format!(
            "KlpMine Gameplay Debug\n\
             F3 main | Shift+M main | Shift+L light | Shift+P performance\n\
             HP: {:.1}\n\
             Blocks: broken {} / placed {}\n\
             Picked items: {}\n\
             Last mass: {:.2}\n\
             Water changes: {}\n\
             Hot reload: {}\n\
             Atlas: {}\n\
             Seed: {}\n\
             CPU: {} | Cores: {}",
            hp,
            broken_blocks,
            placed_blocks,
            picked_items,
            last_mass,
            water_stats
                .map(|stats| stats.last_changes)
                .unwrap_or_default(),
            if hot_reload { "on" } else { "off" },
            if atlas_loaded { "loaded" } else { "pending" },
            seed_value,
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

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
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
