use std::f32::consts::FRAC_PI_2;

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::world::{Chunk, is_solid_at};

pub struct CameraPlugin;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
pub(crate) struct PlayerController {
    yaw: f32,
    pitch: f32,
    position: Vec3,
    vertical_velocity: f32,
    grounded: bool,
    jump_buffer: f32,
    coyote_timer: f32,
    crouch_blend: f32,
    walk_phase: f32,
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
const MOUSE_SENSITIVITY: f32 = 0.0025;
const GROUND_TOP_Y: f32 = 5.0;
const PLAYER_HALF_WIDTH: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const EYE_HEIGHT: f32 = 1.62;
const SNEAK_EYE_DROP: f32 = 0.32;
const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 7.6;
const MAX_FALL_SPEED: f32 = 36.0;
const COLLISION_STEP: f32 = 0.025;
const HEAD_BOB_SPEED: f32 = 8.5;
const HEAD_BOB_HEIGHT: f32 = 0.04;
const HEAD_BOB_SWAY: f32 = 0.02;
const JUMP_BUFFER_TIME: f32 = 0.14;
const COYOTE_TIME: f32 = 0.09;
const CROUCH_LERP_SPEED: f32 = 12.0;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_camera, spawn_crosshair, grab_cursor))
            .add_systems(Update, (toggle_cursor, look_around, walk_player));
    }
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

fn spawn_camera(mut commands: Commands) {
    let yaw = -0.55;
    let pitch = -0.12;

    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: 85.0_f32.to_radians(),
            ..default()
        }),
        Transform {
            translation: Vec3::new(0.0, GROUND_TOP_Y + EYE_HEIGHT, 8.0),
            rotation: Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0),
            ..default()
        },
        PlayerCamera,
        PlayerController {
            yaw,
            pitch,
            position: Vec3::new(0.0, GROUND_TOP_Y, 8.0),
            vertical_velocity: 0.0,
            grounded: false,
            jump_buffer: 0.0,
            coyote_timer: 0.0,
            crouch_blend: 0.0,
            walk_phase: 0.0,
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.1, -0.65, 0.0)),
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
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-9),
                    top: px(0),
                    width: px(18),
                    height: px(2),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.85)),
            ));
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(-9),
                    width: px(2),
                    height: px(18),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.85)),
            ));
        });
}

fn grab_cursor(mut cursor_options: Single<&mut CursorOptions>) {
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

fn toggle_cursor(
    mut cursor_options: Single<&mut CursorOptions>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
    }

    if mouse.just_pressed(MouseButton::Left) {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    }
}

fn look_around(
    cursor_options: Single<&CursorOptions>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut cameras: Query<(&mut Transform, &mut PlayerController), With<PlayerCamera>>,
) {
    if cursor_options.grab_mode == CursorGrabMode::None {
        return;
    }

    let Ok((mut transform, mut controller)) = cameras.single_mut() else {
        return;
    };

    let delta = mouse_motion.delta;
    if delta == Vec2::ZERO {
        return;
    }

    controller.yaw -= delta.x * MOUSE_SENSITIVITY;
    controller.pitch -= delta.y * MOUSE_SENSITIVITY;
    controller.pitch = controller
        .pitch
        .clamp(-(FRAC_PI_2 - 0.01), FRAC_PI_2 - 0.01);

    transform.rotation = Quat::from_euler(EulerRot::YXZ, controller.yaw, controller.pitch, 0.0);
}

fn walk_player(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut cameras: Query<(&mut Transform, &mut PlayerController), With<PlayerCamera>>,
) {
    let Ok((mut transform, mut controller)) = cameras.single_mut() else {
        return;
    };

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
    let sneaking = keys.pressed(KeyCode::ShiftLeft);
    let sprinting = keys.pressed(KeyCode::ControlLeft) && !sneaking && keys.pressed(KeyCode::KeyW);
    let speed = if sneaking {
        SNEAK_SPEED
    } else if sprinting {
        SPRINT_SPEED
    } else {
        WALK_SPEED
    };
    let target_crouch = if sneaking { 1.0 } else { 0.0 };
    let crouch_step = (CROUCH_LERP_SPEED * dt).min(1.0);
    controller.crouch_blend += (target_crouch - controller.crouch_blend) * crouch_step;
    controller.jump_buffer = (controller.jump_buffer - dt).max(0.0);
    controller.coyote_timer = (controller.coyote_timer - dt).max(0.0);
    if controller.grounded {
        controller.coyote_timer = COYOTE_TIME;
    }
    if keys.just_pressed(KeyCode::Space) {
        controller.jump_buffer = JUMP_BUFFER_TIME;
    }

    let is_walking = direction.length_squared() > 0.0 && controller.grounded;

    if controller.jump_buffer > 0.0 && controller.coyote_timer > 0.0 && !sneaking {
        controller.vertical_velocity = JUMP_SPEED;
        controller.grounded = false;
        controller.coyote_timer = 0.0;
        controller.jump_buffer = 0.0;
    }

    if direction.length_squared() > 0.0 {
        let movement = direction.normalize() * speed * dt;
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

    if is_walking {
        controller.walk_phase += HEAD_BOB_SPEED * dt;
    }

    let bob_y = if is_walking {
        controller.walk_phase.sin().abs() * HEAD_BOB_HEIGHT
    } else {
        0.0
    };
    let bob_x = if is_walking {
        controller.walk_phase.cos() * HEAD_BOB_SWAY
    } else {
        0.0
    };
    let crouch_drop = SNEAK_EYE_DROP * controller.crouch_blend;

    transform.translation =
        controller.position + Vec3::Y * (EYE_HEIGHT - crouch_drop + bob_y) + right * bob_x;
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
