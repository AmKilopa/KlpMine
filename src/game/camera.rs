use std::f32::consts::FRAC_PI_2;

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

pub struct CameraPlugin;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
pub(crate) struct PlayerController {
    yaw: f32,
    pitch: f32,
    position: Vec3,
    walk_phase: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerView {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

const WALK_SPEED: f32 = 7.0;
const MOUSE_SENSITIVITY: f32 = 0.0025;
const GROUND_TOP_Y: f32 = 5.0;
const EYE_HEIGHT: f32 = 1.7;
const HEAD_BOB_SPEED: f32 = 9.5;
const HEAD_BOB_HEIGHT: f32 = 0.055;
const HEAD_BOB_SWAY: f32 = 0.028;

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

    let is_walking = direction.length_squared() > 0.0;

    if direction.length_squared() > 0.0 {
        controller.position += direction.normalize() * WALK_SPEED * time.delta_secs();
        controller.walk_phase += HEAD_BOB_SPEED * time.delta_secs();
    } else {
        controller.walk_phase = 0.0;
    }

    controller.position.y = GROUND_TOP_Y;

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

    transform.translation = controller.position + Vec3::Y * (EYE_HEIGHT + bob_y) + right * bob_x;
}
