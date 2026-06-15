use std::f32::consts::FRAC_PI_2;

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::{
    audio::optional_sound,
    chat::{ChatState, is_open as chat_open},
    events::{PlayerDamaged, PlayerRespawned},
    health::PlayerHealth,
    settings::{GameSettings, SettingsState, is_open},
    sky::LightingState,
    world::{Chunk, is_solid_at, player_spawn_position},
};

pub struct CameraPlugin;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
struct PlayerShadowBody;

#[derive(Component)]
struct CrosshairLine {
    primary: bool,
}

#[derive(Component)]
pub(crate) struct PlayerController {
    yaw: f32,
    pitch: f32,
    position: Vec3,
    horizontal_velocity: Vec3,
    vertical_velocity: f32,
    grounded: bool,
    jump_buffer: f32,
    coyote_timer: f32,
    crouch_blend: f32,
    bob_blend: f32,
    walk_phase: f32,
    step_timer: f32,
    step_index: usize,
    fall_start_y: f32,
    was_grounded: bool,
}

impl PlayerController {
    pub fn walk_phase(&self) -> f32 {
        self.walk_phase
    }

    pub fn horizontal_speed(&self) -> f32 {
        self.horizontal_velocity.length() / SPRINT_SPEED
    }

    pub fn vertical_speed(&self) -> f32 {
        self.vertical_velocity
    }

    pub fn crouch_amount(&self) -> f32 {
        self.crouch_blend
    }
}

#[derive(Resource)]
struct MovementAudio {
    steps: Vec<Handle<AudioSource>>,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerView {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

const WALK_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 5.7;
const SNEAK_SPEED: f32 = 1.65;
const GROUND_ACCEL: f32 = 18.0;
const AIR_ACCEL: f32 = 9.0;
const PLAYER_HALF_WIDTH: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const EYE_HEIGHT: f32 = 1.62;
const SNEAK_EYE_DROP: f32 = 0.32;
const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 7.6;
const SPRINT_JUMP_BOOST: f32 = 1.15;
const MAX_FALL_SPEED: f32 = 36.0;
const COLLISION_STEP: f32 = 0.025;
const HEAD_BOB_SPEED: f32 = 8.5;
const HEAD_BOB_HEIGHT: f32 = 0.032;
const HEAD_BOB_SWAY: f32 = 0.015;
const STEP_WALK_INTERVAL: f32 = 0.48;
const STEP_SPRINT_INTERVAL: f32 = 0.34;
const STEP_SNEAK_INTERVAL: f32 = 0.72;
const JUMP_BUFFER_TIME: f32 = 0.14;
const COYOTE_TIME: f32 = 0.09;
const CROUCH_LERP_SPEED: f32 = 12.0;
const SAFE_FALL_DISTANCE: f32 = 3.0;
const FALL_DAMAGE_SCALE: f32 = 1.0;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                setup_movement_audio,
                spawn_camera,
                spawn_player_shadow,
                spawn_crosshair,
                grab_cursor,
            ),
        )
        .add_systems(
            Update,
            (
                toggle_cursor,
                look_around,
                walk_player,
                respawn_player,
                update_player_shadow,
                update_crosshair_color,
                apply_fov,
            ),
        );
    }
}

fn setup_movement_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    let steps = [
        "sounds/step_dirt_1.ogg",
        "sounds/step_dirt_2.ogg",
        "sounds/step_dirt_3.ogg",
        "sounds/step_dirt_4.ogg",
    ]
    .into_iter()
    .filter_map(|path| optional_sound(&asset_server, path))
    .collect();

    commands.insert_resource(MovementAudio { steps });
}

pub fn player_view(
    cameras: &Query<(&Transform, &PlayerController), With<PlayerCamera>>,
) -> Option<PlayerView> {
    cameras
        .single()
        .ok()
        .map(|(transform, controller)| PlayerView {
            position: transform.translation,
            yaw: controller.yaw,
            pitch: controller.pitch,
        })
}

pub fn player_intersects_block(block: IVec3, controller: &PlayerController) -> bool {
    let player_min = Vec3::new(
        controller.position.x - PLAYER_HALF_WIDTH,
        controller.position.y + 0.001,
        controller.position.z - PLAYER_HALF_WIDTH,
    );
    let player_max = Vec3::new(
        controller.position.x + PLAYER_HALF_WIDTH,
        controller.position.y + PLAYER_HEIGHT - 0.001,
        controller.position.z + PLAYER_HALF_WIDTH,
    );
    let block_min = block.as_vec3();
    let block_max = block_min + Vec3::ONE;

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

fn spawn_camera(mut commands: Commands) {
    let yaw = -0.55;
    let pitch = -0.12;
    let spawn = player_spawn_position();

    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: 85.0_f32.to_radians(),
            ..default()
        }),
        Transform {
            translation: spawn + Vec3::Y * EYE_HEIGHT,
            rotation: Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0),
            ..default()
        },
        PlayerCamera,
        PlayerController {
            yaw,
            pitch,
            position: spawn,
            horizontal_velocity: Vec3::ZERO,
            vertical_velocity: 0.0,
            grounded: false,
            jump_buffer: 0.0,
            coyote_timer: 0.0,
            crouch_blend: 0.0,
            bob_blend: 0.0,
            walk_phase: 0.0,
            step_timer: 0.0,
            step_index: 0,
            fall_start_y: spawn.y,
            was_grounded: false,
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 24_000.0,
            shadows_enabled: true,
            shadow_depth_bias: 0.08,
            shadow_normal_bias: 0.65,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, -0.85, 0.0)),
    ));
}

fn spawn_player_shadow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.28),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    let spawn = player_spawn_position();

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(0.7, 0.44))),
        MeshMaterial3d(material),
        Transform::from_translation(spawn + Vec3::Y * 0.012),
        PlayerShadowBody,
    ));
}

fn spawn_crosshair(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(1),
            height: px(1),
            ..default()
        })
        .with_children(|parent| {
            crosshair_arm(parent, -11.0, -1.0, 7.0, 2.0, false);
            crosshair_arm(parent, 4.0, -1.0, 7.0, 2.0, false);
            crosshair_arm(parent, -1.0, -11.0, 2.0, 7.0, false);
            crosshair_arm(parent, -1.0, 4.0, 2.0, 7.0, false);
            crosshair_arm(parent, -10.0, 0.0, 6.0, 1.0, true);
            crosshair_arm(parent, 4.0, 0.0, 6.0, 1.0, true);
            crosshair_arm(parent, 0.0, -10.0, 1.0, 6.0, true);
            crosshair_arm(parent, 0.0, 4.0, 1.0, 6.0, true);
        });
}

fn crosshair_arm(
    parent: &mut ChildSpawnerCommands,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    primary: bool,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(width),
            height: px(height),
            ..default()
        },
        BackgroundColor(Color::WHITE),
        CrosshairLine { primary },
    ));
}

fn update_crosshair_color(
    cameras: Query<&Transform, With<PlayerCamera>>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    lighting: Option<Res<LightingState>>,
    mut lines: Query<(&CrosshairLine, &mut BackgroundColor)>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };

    let day = lighting.as_ref().map(|l| l.day_factor).unwrap_or(0.7);
    let surface_light =
        crosshair_background_light(camera.translation, *camera.forward(), &chunks).unwrap_or(0.7);
    let background_light = surface_light * (0.24 + day * 0.76);
    let primary_color = if background_light < 0.54 {
        Color::srgba(1.0, 1.0, 1.0, 0.92)
    } else {
        Color::srgba(0.02, 0.02, 0.02, 0.86)
    };
    let outline_color = if background_light < 0.54 {
        Color::srgba(0.0, 0.0, 0.0, 0.38)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.32)
    };

    for (line, mut color) in &mut lines {
        *color = BackgroundColor(if line.primary {
            primary_color
        } else {
            outline_color
        });
    }
}

fn grab_cursor(mut cursor_options: Single<&mut CursorOptions>) {
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

fn toggle_cursor(
    settings_state: Res<SettingsState>,
    mut cursor_options: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    if !is_open(&settings_state) && mouse.just_pressed(MouseButton::Left) {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    }
}

fn look_around(
    settings: Res<GameSettings>,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    health: Res<PlayerHealth>,
    cursor_options: Single<&CursorOptions>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut cameras: Query<(&mut Transform, &mut PlayerController), With<PlayerCamera>>,
) {
    if health.dead
        || is_open(&settings_state)
        || chat_open(&chat_state)
        || cursor_options.grab_mode == CursorGrabMode::None
    {
        return;
    }

    let Ok((mut transform, mut controller)) = cameras.single_mut() else {
        return;
    };

    let delta = mouse_motion.delta;
    if delta == Vec2::ZERO {
        return;
    }

    controller.yaw -= delta.x * settings.mouse_sensitivity;
    controller.pitch -= delta.y * settings.mouse_sensitivity;
    controller.pitch = controller
        .pitch
        .clamp(-(FRAC_PI_2 - 0.01), FRAC_PI_2 - 0.01);

    transform.rotation = Quat::from_euler(EulerRot::YXZ, controller.yaw, controller.pitch, 0.0);
}

fn walk_player(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    audio: Res<MovementAudio>,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    health: Res<PlayerHealth>,
    mut damage_events: MessageWriter<PlayerDamaged>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut cameras: Query<(&mut Transform, &mut PlayerController), With<PlayerCamera>>,
) {
    let Ok((mut transform, mut controller)) = cameras.single_mut() else {
        return;
    };

    if health.dead {
        controller.horizontal_velocity = Vec3::ZERO;
        controller.vertical_velocity = 0.0;
        controller.step_timer = 0.0;
        transform.translation = eye_position(&controller);
        return;
    }

    if is_open(&settings_state) || chat_open(&chat_state) {
        controller.horizontal_velocity = Vec3::ZERO;
        controller.step_timer = 0.0;
        transform.translation = eye_position(&controller);
        return;
    }

    let forward = Vec3::new(-controller.yaw.sin(), 0.0, -controller.yaw.cos()).normalize();
    let right = Vec3::new(controller.yaw.cos(), 0.0, -controller.yaw.sin()).normalize();
    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction -= right;
    }

    let dt = time.delta_secs().min(0.05);
    controller.was_grounded = controller.grounded;

    let sneaking = keys.pressed(KeyCode::ShiftLeft);
    let sprinting = keys.pressed(KeyCode::ControlLeft) && !sneaking && keys.pressed(KeyCode::KeyW);
    let target_speed = if sneaking {
        SNEAK_SPEED
    } else if sprinting {
        SPRINT_SPEED
    } else {
        WALK_SPEED
    };

    controller.crouch_blend += (if sneaking { 1.0 } else { 0.0 } - controller.crouch_blend)
        * (CROUCH_LERP_SPEED * dt).min(1.0);
    controller.jump_buffer = (controller.jump_buffer - dt).max(0.0);
    controller.coyote_timer = (controller.coyote_timer - dt).max(0.0);

    if controller.grounded {
        controller.coyote_timer = COYOTE_TIME;
    }
    if keys.just_pressed(KeyCode::Space) {
        controller.jump_buffer = JUMP_BUFFER_TIME;
    }

    let has_input = direction.length_squared() > 0.0;
    let target_velocity = if has_input {
        direction.normalize() * target_speed
    } else {
        Vec3::ZERO
    };
    let accel = if controller.grounded {
        GROUND_ACCEL
    } else {
        AIR_ACCEL
    };
    let current_velocity = controller.horizontal_velocity;
    controller.horizontal_velocity += (target_velocity - current_velocity) * (accel * dt).min(1.0);

    let is_walking = controller.horizontal_velocity.length_squared() > 0.05 && controller.grounded;

    if controller.jump_buffer > 0.0 && controller.coyote_timer > 0.0 {
        controller.vertical_velocity = JUMP_SPEED;
        if sprinting {
            controller.horizontal_velocity += forward * SPRINT_JUMP_BOOST;
        }
        controller.grounded = false;
        controller.coyote_timer = 0.0;
        controller.jump_buffer = 0.0;
    }

    if controller.horizontal_velocity.length_squared() > 0.0001 {
        let movement = controller.horizontal_velocity * dt;
        move_axis(&mut controller.position, Vec3::X * movement.x, &chunks);
        move_axis(&mut controller.position, Vec3::Z * movement.z, &chunks);
    } else {
        controller.walk_phase = 0.0;
    }

    controller.vertical_velocity =
        (controller.vertical_velocity - GRAVITY * dt).max(-MAX_FALL_SPEED);
    controller.grounded = false;

    let vertical_step = Vec3::Y * controller.vertical_velocity * dt;
    let hit_vertical = move_axis(&mut controller.position, vertical_step, &chunks);

    if hit_vertical {
        if vertical_step.y < 0.0 {
            controller.grounded = true;
        }
        controller.vertical_velocity = 0.0;
    }

    if controller.was_grounded && !controller.grounded && controller.vertical_velocity <= 0.0 {
        controller.fall_start_y = controller.position.y;
    }

    if !controller.was_grounded && controller.grounded {
        let fall_distance = (controller.fall_start_y - controller.position.y).max(0.0);
        let damage = fall_damage(fall_distance);
        if damage > 0.0 {
            damage_events.write(PlayerDamaged { amount: damage });
        }
    }

    if is_walking {
        controller.bob_blend += (1.0 - controller.bob_blend) * (1.0 - (-10.0 * dt).exp());
        controller.walk_phase += HEAD_BOB_SPEED * dt;
        controller.step_timer -= dt;

        if controller.step_timer <= 0.0 && !audio.steps.is_empty() {
            commands.spawn((
                AudioPlayer::new(audio.steps[controller.step_index].clone()),
                PlaybackSettings::DESPAWN,
            ));
            controller.step_index = (controller.step_index + 1) % audio.steps.len();
            controller.step_timer = if sneaking {
                STEP_SNEAK_INTERVAL
            } else if sprinting {
                STEP_SPRINT_INTERVAL
            } else {
                STEP_WALK_INTERVAL
            };
        }
    } else {
        controller.bob_blend += (0.0 - controller.bob_blend) * (1.0 - (-12.0 * dt).exp());
        controller.step_timer = 0.0;
    }

    let bob_y = if is_walking {
        controller.walk_phase.sin().abs() * HEAD_BOB_HEIGHT * controller.bob_blend
    } else {
        0.0
    };
    let bob_x = if is_walking {
        controller.walk_phase.cos() * HEAD_BOB_SWAY * controller.bob_blend
    } else {
        0.0
    };

    transform.translation = eye_position(&controller) + Vec3::Y * bob_y + right * bob_x;
}

fn eye_position(controller: &PlayerController) -> Vec3 {
    controller.position + Vec3::Y * (EYE_HEIGHT - SNEAK_EYE_DROP * controller.crouch_blend)
}

fn respawn_player(
    mut respawned: MessageReader<PlayerRespawned>,
    mut cameras: Query<(&mut Transform, &mut PlayerController), With<PlayerCamera>>,
) {
    if respawned.read().next().is_none() {
        return;
    }

    let Ok((mut transform, mut controller)) = cameras.single_mut() else {
        return;
    };

    let spawn = player_spawn_position();

    controller.position = spawn;
    controller.horizontal_velocity = Vec3::ZERO;
    controller.vertical_velocity = 0.0;
    controller.grounded = false;
    controller.jump_buffer = 0.0;
    controller.coyote_timer = 0.0;
    controller.crouch_blend = 0.0;
    controller.bob_blend = 0.0;
    controller.walk_phase = 0.0;
    controller.step_timer = 0.0;
    controller.fall_start_y = spawn.y;
    controller.was_grounded = false;
    transform.translation = spawn + Vec3::Y * EYE_HEIGHT;
    transform.rotation = Quat::from_euler(EulerRot::YXZ, controller.yaw, controller.pitch, 0.0);
}

fn update_player_shadow(
    cameras: Query<&PlayerController, With<PlayerCamera>>,
    mut bodies: Query<&mut Transform, With<PlayerShadowBody>>,
) {
    let Ok(controller) = cameras.single() else {
        return;
    };

    let rotation = Quat::from_rotation_y(controller.yaw);

    for mut transform in &mut bodies {
        transform.translation = controller.position + Vec3::new(0.0, 0.012, 0.0);
        transform.rotation = rotation;
    }
}

fn apply_fov(settings: Res<GameSettings>, mut cameras: Query<&mut Projection, With<PlayerCamera>>) {
    if !settings.is_changed() {
        return;
    }

    for mut projection in &mut cameras {
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = settings.fov.to_radians();
        }
    }
}

fn fall_damage(distance: f32) -> f32 {
    if distance <= SAFE_FALL_DISTANCE {
        return 0.0;
    }
    ((distance - SAFE_FALL_DISTANCE) * FALL_DAMAGE_SCALE * 2.0).ceil() / 2.0
}

fn move_axis(position: &mut Vec3, delta: Vec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> bool {
    if delta.length_squared() == 0.0 {
        return false;
    }

    let distance = delta.length();
    let direction = delta / distance;
    let steps = (distance / COLLISION_STEP).ceil().max(1.0) as usize;
    let step = delta / steps as f32;
    let mut collided = false;

    for _ in 0..steps {
        let next = *position + step;
        if player_collides(next, chunks) {
            collided = true;
            break;
        }
        *position = next;
    }

    let remaining = delta - step * steps as f32;
    if !collided && remaining.dot(direction).abs() > 0.0 {
        let next = *position + remaining;
        if player_collides(next, chunks) {
            collided = true;
        } else {
            *position = next;
        }
    }

    collided
}

fn player_collides(position: Vec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> bool {
    let min = Vec3::new(
        position.x - PLAYER_HALF_WIDTH,
        position.y + 0.001,
        position.z - PLAYER_HALF_WIDTH,
    );
    let max = Vec3::new(
        position.x + PLAYER_HALF_WIDTH,
        position.y + PLAYER_HEIGHT - 0.001,
        position.z + PLAYER_HALF_WIDTH,
    );

    for y in min.y.floor() as i32..=max.y.floor() as i32 {
        for z in min.z.floor() as i32..=max.z.floor() as i32 {
            for x in min.x.floor() as i32..=max.x.floor() as i32 {
                if is_solid_at(IVec3::new(x, y, z), chunks) {
                    return true;
                }
            }
        }
    }

    false
}

fn crosshair_background_light(
    origin: Vec3,
    direction: Vec3,
    chunks: &Query<(&Chunk, &GlobalTransform)>,
) -> Option<f32> {
    let direction = direction.normalize_or_zero();
    if direction == Vec3::ZERO {
        return None;
    }

    let mut block = origin.floor().as_ivec3();
    let step = IVec3::new(
        axis_step(direction.x),
        axis_step(direction.y),
        axis_step(direction.z),
    );
    let mut t_max = Vec3::new(
        first_axis_distance(origin.x, direction.x, step.x),
        first_axis_distance(origin.y, direction.y, step.y),
        first_axis_distance(origin.z, direction.z, step.z),
    );
    let t_delta = Vec3::new(
        axis_delta(direction.x),
        axis_delta(direction.y),
        axis_delta(direction.z),
    );
    let mut traveled = 0.0f32;
    let mut normal = IVec3::Y;

    while traveled <= 7.0 {
        if is_solid_at(block, chunks) {
            return Some(face_background_light(normal));
        }

        if t_max.x <= t_max.y && t_max.x <= t_max.z {
            block.x += step.x;
            traveled = t_max.x;
            t_max.x += t_delta.x;
            normal = IVec3::new(-step.x, 0, 0);
        } else if t_max.y <= t_max.z {
            block.y += step.y;
            traveled = t_max.y;
            t_max.y += t_delta.y;
            normal = IVec3::new(0, -step.y, 0);
        } else {
            block.z += step.z;
            traveled = t_max.z;
            t_max.z += t_delta.z;
            normal = IVec3::new(0, 0, -step.z);
        }
    }

    None
}

fn face_background_light(normal: IVec3) -> f32 {
    match normal {
        IVec3::Y => 0.82,
        IVec3::NEG_Y => 0.18,
        IVec3::X => 0.55,
        IVec3::NEG_X => 0.45,
        IVec3::Z => 0.62,
        IVec3::NEG_Z => 0.34,
        _ => 0.5,
    }
}

fn axis_step(value: f32) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn first_axis_distance(origin: f32, direction: f32, step: i32) -> f32 {
    if step > 0 {
        ((origin.floor() + 1.0) - origin) / direction
    } else if step < 0 {
        (origin - origin.floor()) / -direction
    } else {
        f32::INFINITY
    }
}

fn axis_delta(direction: f32) -> f32 {
    if direction == 0.0 {
        f32::INFINITY
    } else {
        (1.0 / direction).abs()
    }
}
