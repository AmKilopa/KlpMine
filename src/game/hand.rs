use bevy::{ecs::system::SystemParam, prelude::*};

use crate::game::{
    camera::{PlayerCamera, PlayerController},
    events::{BlockBroken, BlockDamaged, BlockPlaced},
    inventory::Inventory,
    settings::{SettingsState, is_open},
    world::{Block, BlockMaterials, FallingTree, active_tree_drag_anchor, build_item_mesh},
};

pub struct HandPlugin;

#[derive(Component)]
struct HandRoot;

#[derive(Component)]
struct HandArm;

#[derive(Component)]
struct HeldBlock {
    current: Option<Block>,
}

#[derive(Resource, Default)]
struct HandMotion {
    swing: f32,
    place: f32,
    break_pulse: f32,
    damage_hold: f32,
    damage_phase: f32,
}

const ARM_BASE: Vec3 = Vec3::new(0.34, -0.34, -0.44);
const BLOCK_BASE: Vec3 = Vec3::new(0.38, -0.25, -0.62);
const ARM_ROTATION: Vec3 = Vec3::new(-0.32, 0.2, -0.18);
const BLOCK_ROTATION: Vec3 = Vec3::new(-0.42, 0.55, -0.08);

type PlayerHandQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, &'static PlayerController),
    (
        With<PlayerCamera>,
        Without<HandArm>,
        Without<HeldBlock>,
        Without<FallingTree>,
    ),
>;
type TreeGripQuery<'w, 's> = Query<
    'w,
    's,
    (&'static FallingTree, &'static Transform),
    (Without<PlayerCamera>, Without<HandArm>, Without<HeldBlock>),
>;
type HandArmQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<HandArm>,
        Without<HeldBlock>,
        Without<PlayerCamera>,
        Without<FallingTree>,
    ),
>;
type HeldBlockQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut Mesh3d,
        &'static mut HeldBlock,
    ),
    (
        With<HeldBlock>,
        Without<HandArm>,
        Without<PlayerCamera>,
        Without<FallingTree>,
    ),
>;
#[derive(SystemParam)]
struct HandQueries<'w, 's> {
    players: PlayerHandQuery<'w, 's>,
    trees: TreeGripQuery<'w, 's>,
    arms: HandArmQuery<'w, 's>,
    held_blocks: HeldBlockQuery<'w, 's>,
}

impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HandMotion::default()).add_systems(
            Update,
            (spawn_hand, update_hand_motion, update_hand).chain(),
        );
    }
}

fn spawn_hand(
    mut commands: Commands,
    cameras: Query<Entity, With<PlayerCamera>>,
    existing: Query<Entity, With<HandRoot>>,
    block_materials: Option<Res<BlockMaterials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !existing.is_empty() {
        return;
    }

    let Some(block_materials) = block_materials else {
        return;
    };
    let Ok(camera) = cameras.single() else {
        return;
    };

    let arm_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.55, 0.4),
        perceptual_roughness: 0.92,
        ..default()
    });

    commands.entity(camera).with_children(|parent| {
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.16, 0.16, 0.52))),
            MeshMaterial3d(arm_material),
            Transform::from_translation(ARM_BASE).with_rotation(Quat::from_euler(
                EulerRot::XYZ,
                ARM_ROTATION.x,
                ARM_ROTATION.y,
                ARM_ROTATION.z,
            )),
            HandRoot,
            HandArm,
        ));
        parent.spawn((
            Mesh3d(meshes.add(build_item_mesh(Block::Dirt))),
            MeshMaterial3d(block_materials.held_terrain.clone()),
            Transform::from_translation(BLOCK_BASE)
                .with_scale(Vec3::splat(0.23))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    BLOCK_ROTATION.x,
                    BLOCK_ROTATION.y,
                    BLOCK_ROTATION.z,
                )),
            Visibility::Hidden,
            HeldBlock { current: None },
        ));
    });
}

fn update_hand_motion(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: ResMut<HandMotion>,
    mut damaged: MessageReader<BlockDamaged>,
    mut placed: MessageReader<BlockPlaced>,
    mut broken: MessageReader<BlockBroken>,
) {
    let dt = time.delta_secs();
    motion.swing = (motion.swing - dt * 4.8).max(0.0);
    motion.place = (motion.place - dt * 6.5).max(0.0);
    motion.break_pulse = (motion.break_pulse - dt * 9.0).max(0.0);
    motion.damage_hold = (motion.damage_hold - dt).max(0.0);
    motion.damage_phase += dt * 18.0;

    if mouse.just_pressed(MouseButton::Left) {
        motion.swing = motion.swing.max(0.72);
    }

    for event in damaged.read() {
        motion.damage_hold = 0.16;
        motion.break_pulse = motion.break_pulse.max(0.16 + event.progress * 0.12);
        motion.swing = motion.swing.max(0.18);
    }

    for _ in placed.read() {
        motion.place = 1.0;
        motion.swing = motion.swing.max(0.75);
    }

    for _ in broken.read() {
        motion.swing = 1.0;
        motion.break_pulse = 0.55;
    }
}

fn update_hand(
    settings_state: Res<SettingsState>,
    mouse: Res<ButtonInput<MouseButton>>,
    inventory: Res<Inventory>,
    motion: Res<HandMotion>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut queries: HandQueries,
) {
    let (camera_matrix, movement, phase, crouch, jump) = {
        let Ok((camera, player)) = queries.players.single() else {
            return;
        };
        (
            camera.to_matrix(),
            player.horizontal_speed().min(1.0),
            player.walk_phase(),
            player.crouch_amount(),
            (player.vertical_speed() / 18.0).clamp(-0.7, 0.7),
        )
    };

    let bob = if is_open(&settings_state) {
        Vec3::ZERO
    } else {
        Vec3::new(phase.cos() * 0.018, phase.sin().abs() * 0.028, 0.0) * movement
    };
    let body = Vec3::new(
        0.0,
        -0.045 * crouch - 0.035 * jump,
        0.035 * crouch - 0.025 * jump,
    );
    let swing = motion.swing;
    let place = motion.place;
    let break_pulse = motion.break_pulse;
    let damage_wave = if motion.damage_hold > 0.0 {
        motion.damage_phase.sin() * 0.045
    } else {
        0.0
    };
    let action = Vec3::new(
        -0.035 * swing + 0.018 * place,
        -0.07 * swing + 0.025 * break_pulse - damage_wave.abs(),
        0.11 * swing - 0.05 * place,
    );
    let grip = if mouse.pressed(MouseButton::Right) {
        active_tree_drag_anchor(&queries.trees)
            .map(|anchor| hand_grip_offset(camera_matrix, anchor))
    } else {
        None
    };
    let grip_offset = grip.unwrap_or(Vec3::ZERO);
    let grip_amount = if grip.is_some() { 1.0 } else { 0.0 };

    for mut arm in &mut queries.arms {
        arm.translation = ARM_BASE + bob + action + body + grip_offset * 0.55;
        arm.rotation = Quat::from_euler(
            EulerRot::XYZ,
            ARM_ROTATION.x + phase.sin() * 0.025 * movement - swing * 0.42 - damage_wave
                + jump * 0.08
                - grip_amount * 0.28,
            ARM_ROTATION.y + place * 0.12 - grip_amount * 0.16,
            ARM_ROTATION.z + phase.cos() * 0.025 * movement + swing * 0.18
                - crouch * 0.05
                - grip_amount * 0.22,
        );
    }

    for (mut transform, mut visibility, mut mesh, mut held) in &mut queries.held_blocks {
        let selected = inventory.selected_block();
        if selected != held.current {
            held.current = selected;
            if let Some(block) = selected {
                *mesh = Mesh3d(meshes.add(build_item_mesh(block)));
            }
        }

        *visibility = if selected.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        transform.translation = BLOCK_BASE + bob * 1.15 + action * 1.05 + body + grip_offset * 0.8;
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            BLOCK_ROTATION.x + phase.sin() * 0.03 * movement - swing * 0.55 - damage_wave * 1.4
                + jump * 0.1
                - grip_amount * 0.36,
            BLOCK_ROTATION.y + place * 0.22 - grip_amount * 0.2,
            BLOCK_ROTATION.z + phase.cos() * 0.04 * movement + swing * 0.2
                - crouch * 0.08
                - grip_amount * 0.24,
        );
    }
}

fn hand_grip_offset(camera_matrix: Mat4, anchor: Vec3) -> Vec3 {
    let local = camera_matrix.inverse().transform_point3(anchor);
    let target = Vec3::new(
        local.x.clamp(0.16, 0.58),
        local.y.clamp(-0.48, -0.1),
        local.z.clamp(-1.0, -0.36),
    );

    (target - BLOCK_BASE).clamp_length_max(0.34)
}
