use std::collections::HashSet;

use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::PrimitiveTopology};

use crate::game::{
    audio::optional_sound,
    camera::{PlayerCamera, PlayerController, player_intersects_block},
    chat::{ChatState, is_open as chat_open},
    debug::PhysicsDebug,
    events::{
        BlockBroken, BlockDamaged, BlockPlaced, ItemDropped, ItemPickedUp, PlayerDamaged,
        PlayerDied,
    },
    inventory::Inventory,
    settings::{GameSettings, SettingsState, is_open},
};

mod block;
mod chunk;
mod generation;
mod materials;
mod meshing;

pub use block::Block;
pub use chunk::Chunk;
pub use materials::BlockMaterials;
pub use meshing::{build_item_mesh, build_log_stack_mesh};

use chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use generation::generate_chunk;
pub use generation::player_spawn_position;
use meshing::build_chunk_mesh_with_neighbors;

pub struct WorldPlugin;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct ChunkCoord(IVec2);

#[derive(Resource)]
struct WorldStreaming {
    chunks_per_tick: usize,
    timer: Timer,
}

#[derive(Resource)]
struct BlockAudio {
    break_sound: Option<Handle<AudioSource>>,
    leaf_sound: Option<Handle<AudioSource>>,
    tree_fall_sound: Option<Handle<AudioSource>>,
}

#[derive(Component)]
struct BreakParticle {
    velocity: Vec3,
    lifetime: Timer,
}

#[derive(Component)]
struct DroppedBlock {
    block: Block,
    velocity: Vec3,
    age: f32,
    pickup_delay: f32,
    mass: f32,
}

#[derive(Component)]
struct FallingBlock {
    block: Block,
    velocity: Vec3,
    start_y: f32,
    damaged_player: bool,
}

#[derive(Component)]
struct FallingTree {
    base: Vec3,
    base_velocity: Vec3,
    direction: Vec3,
    angle: f32,
    angular_speed: f32,
    height: f32,
    mass: f32,
    damaged_player: bool,
    settled: bool,
}

#[derive(Resource, Default)]
struct BreakState {
    target: Option<IVec3>,
    progress: f32,
}

#[derive(Resource)]
struct FallingBlockScan {
    timer: Timer,
}

#[derive(Resource, Default)]
struct ChunkLoadStatus {
    pending: usize,
    phase: usize,
    visible_timer: f32,
}

#[derive(Component)]
struct ChunkLoadText;

const BLOCK_REACH: f32 = 7.0;
const PICKUP_RADIUS: f32 = 1.45;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BreakState::default())
            .insert_resource(WorldStreaming {
                chunks_per_tick: 1,
                timer: Timer::from_seconds(0.16, TimerMode::Repeating),
            })
            .insert_resource(FallingBlockScan {
                timer: Timer::from_seconds(0.35, TimerMode::Repeating),
            })
            .insert_resource(ChunkLoadStatus::default())
            .add_systems(
                Startup,
                (
                    materials::setup_materials,
                    setup_block_audio,
                    spawn_world,
                    spawn_chunk_load_ui,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    stream_world_chunks,
                    break_selected_block,
                    place_selected_block,
                    drop_selected_block,
                    start_falling_blocks,
                    update_falling_blocks,
                    update_falling_trees,
                    drop_inventory_on_death,
                    update_dropped_blocks,
                    pickup_dropped_blocks,
                    update_break_particles,
                    update_block_selection,
                    update_physics_debug,
                    update_chunk_load_ui,
                )
                    .chain(),
            );
    }
}

fn setup_block_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(BlockAudio {
        break_sound: optional_sound(&asset_server, "sounds/block_break_dirt.ogg"),
        leaf_sound: optional_sound(&asset_server, "sounds/leaves_decay.ogg"),
        tree_fall_sound: optional_sound(&asset_server, "sounds/tree_fall.ogg"),
    });
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    let radius = 2;
    let mut generated_chunks = Vec::new();

    for chunk_x in -radius..=radius {
        for chunk_z in -radius..=radius {
            let origin = IVec3::new(chunk_x * CHUNK_SIZE as i32, 0, chunk_z * CHUNK_SIZE as i32);
            generated_chunks.push((origin, generate_chunk(IVec2::new(chunk_x, chunk_z))));
        }
    }

    for (origin, chunk) in &generated_chunks {
        let Some(mesh) = build_chunk_mesh_with_neighbors(chunk, |local| {
            block_from_snapshot(*origin + local, &generated_chunks)
        }) else {
            continue;
        };

        spawn_chunk_entity(
            &mut commands,
            &mut meshes,
            &materials,
            *origin,
            chunk.clone(),
            mesh,
        );
    }
}

fn stream_world_chunks(
    mut commands: Commands,
    time: Res<Time>,
    materials: Res<BlockMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut streaming: ResMut<WorldStreaming>,
    mut status: ResMut<ChunkLoadStatus>,
    settings: Res<GameSettings>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    chunks: Query<(Entity, &ChunkCoord, &Chunk, &GlobalTransform)>,
) {
    streaming.timer.tick(time.delta());

    if !streaming.timer.just_finished() {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let player_chunk = world_chunk_coord(camera.translation.floor().as_ivec3());
    let render_distance = settings.render_distance.clamp(2, 7);
    let unload_distance = render_distance + 1;
    let forward = Vec2::new(camera.forward().x, camera.forward().z).normalize_or_zero();
    let loaded: HashSet<IVec2> = chunks.iter().map(|(_, coord, _, _)| coord.0).collect();
    let mut missing = Vec::new();

    for x in player_chunk.x - render_distance..=player_chunk.x + render_distance {
        for z in player_chunk.y - render_distance..=player_chunk.y + render_distance {
            let coord = IVec2::new(x, z);
            if loaded.contains(&coord) || !chunk_should_load(coord, player_chunk, forward) {
                continue;
            }
            missing.push(coord);
        }
    }

    for (entity, coord, _, _) in &chunks {
        let distance = (coord.0 - player_chunk).abs();
        if distance.x > unload_distance
            || distance.y > unload_distance
            || !chunk_should_keep(coord.0, player_chunk, forward, render_distance)
        {
            commands.entity(entity).despawn();
        }
    }

    if missing.is_empty() {
        status.pending = 0;
        return;
    }

    status.pending = missing.len();
    status.visible_timer = 0.8;
    status.phase = (status.phase + 1) % 4;
    missing.sort_by_key(|coord| chunk_load_score(*coord, player_chunk, forward));

    let mut snapshot: Vec<(IVec3, Chunk)> = chunks
        .iter()
        .map(|(_, _, chunk, transform)| (transform.translation().floor().as_ivec3(), chunk.clone()))
        .collect();

    let generated: Vec<(IVec2, IVec3, Chunk)> = missing
        .into_iter()
        .take(streaming.chunks_per_tick)
        .map(|coord| {
            let origin = chunk_origin(coord);
            let chunk = generate_chunk(coord);
            snapshot.push((origin, chunk.clone()));
            (coord, origin, chunk)
        })
        .collect();

    for (coord, origin, chunk) in generated {
        let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
            block_from_snapshot(origin + local, &snapshot)
        })
        .unwrap_or_else(empty_mesh);
        spawn_chunk_entity(&mut commands, &mut meshes, &materials, origin, chunk, mesh)
            .insert(ChunkCoord(coord));
    }
}

fn spawn_chunk_load_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            right: px(18),
            bottom: px(18),
            ..default()
        },
        GlobalZIndex(i32::MAX - 6),
        Visibility::Hidden,
        ChunkLoadText,
    ));
}

fn update_chunk_load_ui(
    time: Res<Time>,
    mut status: ResMut<ChunkLoadStatus>,
    mut texts: Query<(&mut Text, &mut Visibility), With<ChunkLoadText>>,
) {
    status.visible_timer = (status.visible_timer - time.delta_secs()).max(0.0);
    let Ok((mut text, mut visibility)) = texts.single_mut() else {
        return;
    };

    if status.visible_timer <= 0.0 || status.pending == 0 {
        *visibility = Visibility::Hidden;
        return;
    }

    *visibility = Visibility::Visible;
    let dots = ".".repeat(status.phase + 1);
    text.0 = format!("Loading chunks{} {}", dots, status.pending);
}

fn spawn_chunk_entity<'a>(
    commands: &'a mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    origin: IVec3,
    chunk: Chunk,
    mesh: Mesh,
) -> EntityCommands<'a> {
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.terrain.clone()),
        Transform::from_translation(origin.as_vec3()),
        ChunkCoord(world_chunk_coord(origin)),
        chunk,
    ))
}

fn chunk_should_load(coord: IVec2, player_chunk: IVec2, forward: Vec2) -> bool {
    let offset = coord - player_chunk;
    let distance = offset.abs();

    if distance.x <= 2 && distance.y <= 2 {
        return true;
    }

    if forward == Vec2::ZERO {
        return true;
    }

    let direction = Vec2::new(offset.x as f32, offset.y as f32).normalize_or_zero();
    direction.dot(forward) > -0.15
}

fn chunk_should_keep(
    coord: IVec2,
    player_chunk: IVec2,
    forward: Vec2,
    render_distance: i32,
) -> bool {
    let offset = coord - player_chunk;
    let distance = offset.abs();

    if distance.x <= 2 && distance.y <= 2 {
        return true;
    }

    if distance.x <= render_distance && distance.y <= render_distance {
        return chunk_should_load(coord, player_chunk, forward);
    }

    false
}

fn chunk_load_score(coord: IVec2, player_chunk: IVec2, forward: Vec2) -> i32 {
    let offset = coord - player_chunk;
    let distance = offset.abs();
    let base = distance.x + distance.y;

    if forward == Vec2::ZERO {
        return base * 10;
    }

    let direction = Vec2::new(offset.x as f32, offset.y as f32).normalize_or_zero();
    let front_bonus = (direction.dot(forward) * 6.0).round() as i32;

    base * 10 - front_bonus
}

fn break_selected_block(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    audio: Res<BlockAudio>,
    mut break_state: ResMut<BreakState>,
    mut broken_events: MessageWriter<BlockBroken>,
    mut damaged_events: MessageWriter<BlockDamaged>,
    mut dropped_events: MessageWriter<ItemDropped>,
    mut chunks: Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    if is_open(&settings_state) || chat_open(&chat_state) || !mouse.pressed(MouseButton::Left) {
        break_state.target = None;
        break_state.progress = 0.0;
        return;
    }

    let Ok(camera) = cameras.single() else {
        break_state.target = None;
        break_state.progress = 0.0;
        return;
    };

    let Some(hit) = raycast_blocks_mut(camera.translation, *camera.forward(), &mut chunks) else {
        break_state.target = None;
        break_state.progress = 0.0;
        return;
    };

    if break_state.target == Some(hit.block) {
        break_state.progress += time.delta_secs();
    } else {
        break_state.target = Some(hit.block);
        break_state.progress = 0.0;
    }

    let hit_block = block_at_world_mut(hit.block, &mut chunks);
    let break_time = if hit_block == Block::Log {
        (hit_block.hardness() + connected_log_count(hit.block, &mut chunks) as f32 * 0.22).max(0.1)
    } else {
        hit_block.hardness().max(0.1)
    };

    damaged_events.write(BlockDamaged {
        block: hit_block,
        position: hit.block,
        progress: (break_state.progress / break_time).clamp(0.0, 1.0),
    });

    if break_state.progress < break_time {
        return;
    }

    break_state.target = None;
    break_state.progress = 0.0;

    let mut changed_origin = None;
    let mut changed_local = None;
    let mut tree_to_fall = None;

    for (mut chunk, transform, _) in &mut chunks {
        let local = hit.block - transform.translation().floor().as_ivec3();

        if !Chunk::contains(local) {
            continue;
        }

        let block = chunk.get(local.x, local.y, local.z);
        if !block.is_solid() {
            continue;
        }

        let tree_fall = if block == Block::Log {
            Some(connected_log_span_in_chunk(&chunk, hit.block, local.y))
        } else {
            None
        };

        chunk.set_local(local, Block::Air);
        changed_origin = Some(transform.translation().floor().as_ivec3());
        changed_local = Some(local);

        if let Some(span) = tree_fall {
            tree_to_fall = Some((hit.block, span));
        } else {
            spawn_dropped_block(
                &mut commands,
                &mut meshes,
                &materials,
                block,
                dropped_position(hit.block),
                dropped_velocity(hit.block),
                0.25,
            );
            dropped_events.write(ItemDropped {
                block,
                position: dropped_position(hit.block),
            });
        }

        spawn_break_effect(
            &mut commands,
            &mut meshes,
            &materials,
            hit.block.as_vec3() + Vec3::splat(0.5),
        );

        if let Some(sound) = &audio.break_sound {
            commands.spawn((AudioPlayer::new(sound.clone()), PlaybackSettings::DESPAWN));
        }

        broken_events.write(BlockBroken {
            block,
            position: hit.block,
        });

        break;
    }

    let Some(origin) = changed_origin else {
        return;
    };
    let Some(local) = changed_local else {
        return;
    };

    if let Some((position, span)) = tree_to_fall {
        spawn_falling_tree_from_block(
            &mut commands,
            &mut chunks,
            &mut meshes,
            &materials,
            position,
            span,
            camera.translation,
            &audio,
        );
        rebuild_all_chunks(&mut chunks, &mut meshes);
        return;
    }

    rebuild_changed_chunks(origin, local, &mut chunks, &mut meshes);
}

fn place_selected_block(
    mouse: Res<ButtonInput<MouseButton>>,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    players: Query<&PlayerController, With<PlayerCamera>>,
    mut inventory: ResMut<Inventory>,
    mut placed_events: MessageWriter<BlockPlaced>,
    mut chunks: Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if is_open(&settings_state) || chat_open(&chat_state) || !mouse.just_pressed(MouseButton::Right)
    {
        return;
    }

    let Some(block) = inventory.selected_block() else {
        return;
    };
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok(player) = players.single() else {
        return;
    };
    let Some(hit) = raycast_blocks_mut(camera.translation, *camera.forward(), &mut chunks) else {
        return;
    };

    if hit.normal == IVec3::ZERO {
        return;
    }

    let target = hit.block + hit.normal;
    if block_at_world_mut(target, &mut chunks).is_solid() || player_intersects_block(target, player)
    {
        return;
    }

    if set_block_at_world(target, block, &mut chunks, &mut meshes) {
        inventory.remove_selected();
        placed_events.write(BlockPlaced {
            block,
            position: target,
        });
    }
}

fn drop_selected_block(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut inventory: ResMut<Inventory>,
    mut dropped_events: MessageWriter<ItemDropped>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    if is_open(&settings_state) || chat_open(&chat_state) || !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }

    let Some(block) = inventory.remove_selected() else {
        return;
    };
    let Ok(camera) = cameras.single() else {
        inventory.add(block);
        return;
    };

    let forward = *camera.forward();
    let position = camera.translation + forward * 0.9 + Vec3::new(0.0, -0.35, 0.0);
    spawn_dropped_block(
        &mut commands,
        &mut meshes,
        &materials,
        block,
        position,
        forward * 4.2 + Vec3::Y * 2.0,
        0.75,
    );
    dropped_events.write(ItemDropped { block, position });
}

fn start_falling_blocks(
    mut commands: Commands,
    time: Res<Time>,
    mut scan: ResMut<FallingBlockScan>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut chunks: Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    scan.timer.tick(time.delta());

    if !scan.timer.just_finished() {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let center = camera.translation.floor().as_ivec3();
    let snapshot = chunk_snapshot_immut(&chunks);
    let mut falling = Vec::new();
    let radius = 8;
    let top_y = (center.y + 6).clamp(1, CHUNK_HEIGHT as i32 - 1);

    for z in center.z - radius..=center.z + radius {
        for x in center.x - radius..=center.x + radius {
            for y in (1..=top_y).rev() {
                let world = IVec3::new(x, y, z);
                let block = block_from_snapshot(world, &snapshot);

                if !block.falls() || block_from_snapshot(world + IVec3::NEG_Y, &snapshot).is_solid()
                {
                    continue;
                }

                falling.push((world, block));
                if falling.len() >= 8 {
                    break;
                }
            }
            if falling.len() >= 8 {
                break;
            }
        }
        if falling.len() >= 8 {
            break;
        }
    }

    for (position, block) in falling {
        if set_block_at_world(position, Block::Air, &mut chunks, &mut meshes) {
            spawn_falling_block(
                &mut commands,
                &mut meshes,
                &materials,
                block,
                position.as_vec3() + Vec3::splat(0.5),
            );
        }
    }
}

fn update_falling_blocks(
    mut commands: Commands,
    time: Res<Time>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut damage_events: MessageWriter<PlayerDamaged>,
    mut falling_blocks: Query<(Entity, &mut FallingBlock, &mut Transform), Without<PlayerCamera>>,
    mut chunks: Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.05);
    let player = cameras.single().ok();

    for (entity, mut falling, mut transform) in &mut falling_blocks {
        falling.velocity.y = (falling.velocity.y - 22.0 * dt).max(-38.0);
        transform.translation += falling.velocity * dt;
        transform.rotate_y(0.9 * dt);

        if !falling.damaged_player {
            if let Some(player_transform) = player {
                if falling_hits_player(transform.translation, player_transform.translation) {
                    let impact = falling.velocity.y.abs();
                    let distance = (falling.start_y - transform.translation.y).max(0.0);
                    let damage = ((impact - 5.0).max(0.0) * 0.14 + distance * 0.18).min(8.0);

                    if damage >= 0.5 {
                        damage_events.write(PlayerDamaged {
                            amount: (damage * 2.0).round() / 2.0,
                        });
                        falling.damaged_player = true;
                    }
                }
            }
        }

        let block_pos = transform.translation.floor().as_ivec3();
        let below = block_pos + IVec3::NEG_Y;
        let should_land = falling.velocity.y <= 0.0
            && (block_pos.y <= 0 || block_at_world_mut(below, &mut chunks).is_solid());

        if should_land {
            let target = if block_at_world_mut(block_pos, &mut chunks).is_solid() {
                block_pos + IVec3::Y
            } else {
                block_pos
            };

            if set_block_at_world(target, falling.block, &mut chunks, &mut meshes) {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn spawn_falling_tree_from_block(
    commands: &mut Commands,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    broken: IVec3,
    span: (i32, i32),
    player_position: Vec3,
    audio: &BlockAudio,
) {
    let (bottom, top) = span;
    let height = (top - bottom + 1).max(1);
    let crown = top;

    for y in bottom..=top {
        set_block_direct(IVec3::new(broken.x, y, broken.z), Block::Air, chunks);
    }

    for y in crown - 4..=crown + 5 {
        for z in broken.z - 5..=broken.z + 5 {
            for x in broken.x - 5..=broken.x + 5 {
                let pos = IVec3::new(x, y, z);
                if (x - broken.x).abs() + (z - broken.z).abs() > 7 {
                    continue;
                }
                if block_at_world_mut(pos, chunks) == Block::Leaves {
                    set_block_direct(pos, Block::Air, chunks);
                    spawn_leaf_particle(
                        commands,
                        meshes,
                        materials,
                        pos.as_vec3() + Vec3::splat(0.5),
                    );
                }
            }
        }
    }

    let base = Vec3::new(broken.x as f32 + 0.5, bottom as f32, broken.z as f32 + 0.5);
    let tree_center = base + Vec3::Y * (height as f32 * 0.5);
    let mut fall_dir = tree_center - player_position;
    fall_dir.y = 0.0;
    let fall_dir = fall_dir.normalize_or_zero();
    let fall_dir = if fall_dir == Vec3::ZERO {
        Vec3::X
    } else {
        fall_dir
    };
    let mass = height as f32 * Block::Log.mass();

    commands.spawn((
        Mesh3d(meshes.add(build_log_stack_mesh(height))),
        MeshMaterial3d(materials.log_physics.clone()),
        Transform::from_translation(tree_center),
        FallingTree {
            base,
            base_velocity: fall_dir * 0.45,
            direction: fall_dir,
            angle: 0.02,
            angular_speed: 0.08,
            height: height as f32,
            mass,
            damaged_player: false,
            settled: false,
        },
    ));

    if let Some(sound) = &audio.leaf_sound {
        commands.spawn((AudioPlayer::new(sound.clone()), PlaybackSettings::DESPAWN));
    }
}

fn update_falling_trees(
    mut commands: Commands,
    time: Res<Time>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    audio: Res<BlockAudio>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut damage_events: MessageWriter<PlayerDamaged>,
    mut trees: Query<(&mut FallingTree, &mut Transform), Without<PlayerCamera>>,
) {
    let dt = time.delta_secs().min(0.04);
    let player = cameras.single().ok();

    for (mut tree, mut transform) in &mut trees {
        if !tree.settled {
            tree.angular_speed = (tree.angular_speed + (0.3 + tree.mass * 0.008) * dt).min(0.95);
            tree.angle = (tree.angle + tree.angular_speed * dt).min(1.54);
            tree.base_velocity.y = (tree.base_velocity.y - 4.2 * dt).max(-8.0);
            let base_velocity = tree.base_velocity;
            tree.base += base_velocity * dt;
        } else {
            tree.base_velocity *= 1.0 - (1.5 * dt).min(0.8);
        }

        let axis = tree_axis(&tree);
        transform.translation = tree.base + axis * (tree.height * 0.5);
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, axis);

        if let Some(player) = player {
            let point = closest_point_on_tree(&tree, player.translation);
            let offset = point - player.translation;
            let horizontal = Vec3::new(offset.x, 0.0, offset.z);

            if horizontal.length() < 0.85 && offset.y.abs() < 1.55 {
                let push = Vec3::new(
                    player.translation.x - point.x,
                    0.0,
                    player.translation.z - point.z,
                )
                .normalize_or_zero();
                let mass = tree.mass.max(1.0);
                tree.base_velocity -= push * (2.8 / mass);

                if !tree.damaged_player
                    && (tree.angular_speed > 0.85 || tree.base_velocity.length() > 2.2)
                {
                    damage_events.write(PlayerDamaged {
                        amount: (2.0 + tree.mass * 0.35).min(8.0),
                    });
                    tree.damaged_player = true;
                }
            }
        }

        if let Some(correction) = tree_ground_correction(&tree, &chunks) {
            tree.base.y += correction;
            tree.base_velocity.y = tree.base_velocity.y.max(0.0) * 0.18;
            tree.base_velocity.x *= 0.72;
            tree.base_velocity.z *= 0.72;
            tree.angular_speed *= 0.68;

            if !tree.settled && tree.angle > 1.2 {
                tree.settled = true;
                if let Some(sound) = &audio.tree_fall_sound {
                    commands.spawn((AudioPlayer::new(sound.clone()), PlaybackSettings::DESPAWN));
                }
            }
        }

        if tree.settled && tree.base_velocity.length_squared() < 0.01 && tree.angular_speed < 0.03 {
            tree.base_velocity = Vec3::ZERO;
            tree.angular_speed = 0.0;
        }
    }
}

fn tree_axis(tree: &FallingTree) -> Vec3 {
    (Vec3::Y * tree.angle.cos() + tree.direction * tree.angle.sin()).normalize_or(Vec3::Y)
}

fn closest_point_on_tree(tree: &FallingTree, point: Vec3) -> Vec3 {
    let axis = tree_axis(tree);
    let start = tree.base;
    let end = tree.base + axis * tree.height;
    let length_sq = start.distance_squared(end);

    if length_sq <= f32::EPSILON {
        return start;
    }

    let t = ((point - start).dot(end - start) / length_sq).clamp(0.0, 1.0);
    start.lerp(end, t)
}

fn tree_ground_correction(
    tree: &FallingTree,
    chunks: &Query<(&Chunk, &GlobalTransform)>,
) -> Option<f32> {
    let axis = tree_axis(tree);
    let samples = (tree.height.ceil() as i32).clamp(3, 10);
    let mut correction: f32 = 0.0;
    let mut touched = false;

    for index in 1..=samples {
        let t = index as f32 / samples as f32;
        let point = tree.base + axis * (tree.height * t);
        let side_extent = 0.48 * (1.0 - axis.y.abs());
        let foot = point + Vec3::new(0.0, -side_extent, 0.0);
        let block_pos = foot.floor().as_ivec3();

        if is_solid_at(block_pos, chunks) {
            let top = block_pos.y as f32 + 1.0;
            let needed = top - foot.y + 0.015;

            if needed > correction {
                correction = needed;
            }
            touched = true;
        }
    }

    touched.then_some(correction.max(0.0))
}

fn set_block_direct(
    world_pos: IVec3,
    block: Block,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) {
    for (mut chunk, transform, _) in chunks.iter_mut() {
        let local = world_pos - transform.translation().floor().as_ivec3();
        if Chunk::contains(local) {
            chunk.set_local(local, block);
            return;
        }
    }
}

fn spawn_leaf_particle(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    center: Vec3,
) {
    let mesh = meshes.add(Cuboid::new(0.1, 0.1, 0.1));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.leaf_particle.clone()),
        Transform::from_translation(center),
        BreakParticle {
            velocity: Vec3::new(
                random_unit(center.floor().as_ivec3(), 7) * 1.6 - 0.8,
                1.1 + random_unit(center.floor().as_ivec3(), 9),
                random_unit(center.floor().as_ivec3(), 13) * 1.6 - 0.8,
            ),
            lifetime: Timer::from_seconds(0.65, TimerMode::Once),
        },
    ));
}

fn drop_inventory_on_death(
    mut commands: Commands,
    mut death_events: MessageReader<PlayerDied>,
    mut dropped_events: MessageWriter<ItemDropped>,
    mut inventory: ResMut<Inventory>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    if death_events.read().next().is_none() {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let origin = camera.translation + Vec3::new(0.0, -1.1, 0.0);
    let mut index = 0;

    for stack in inventory.take_all() {
        for _ in 0..stack.count {
            let angle = index as f32 * 2.399;
            let radius = 0.35 + (index % 5) as f32 * 0.08;
            let position = origin + Vec3::new(angle.cos() * radius, 0.2, angle.sin() * radius);
            let velocity = Vec3::new(
                angle.cos() * 2.0,
                2.2 + (index % 4) as f32 * 0.25,
                angle.sin() * 2.0,
            );

            spawn_dropped_block(
                &mut commands,
                &mut meshes,
                &materials,
                stack.block,
                position,
                velocity,
                1.0,
            );
            dropped_events.write(ItemDropped {
                block: stack.block,
                position,
            });
            index += 1;
        }
    }
}

fn spawn_falling_block(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    block: Block,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(build_item_mesh(block))),
        MeshMaterial3d(materials.terrain.clone()),
        Transform::from_translation(position),
        FallingBlock {
            block,
            velocity: Vec3::ZERO,
            start_y: position.y,
            damaged_player: false,
        },
    ));
}

fn falling_hits_player(block_center: Vec3, eye_position: Vec3) -> bool {
    let player_min = Vec3::new(
        eye_position.x - 0.34,
        eye_position.y - 1.62,
        eye_position.z - 0.34,
    );
    let player_max = Vec3::new(
        eye_position.x + 0.34,
        eye_position.y + 0.18,
        eye_position.z + 0.34,
    );
    let block_min = block_center - Vec3::splat(0.5);
    let block_max = block_center + Vec3::splat(0.5);

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

fn spawn_break_effect(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    center: Vec3,
) {
    let particle_mesh = meshes.add(Cuboid::new(0.12, 0.12, 0.12));

    for index in 0..12 {
        let angle = index as f32 * 1.73;
        let radius = 0.18 + (index % 3) as f32 * 0.035;
        let offset = Vec3::new(
            angle.cos() * radius,
            (index % 4) as f32 * 0.05,
            angle.sin() * radius,
        );
        let velocity = Vec3::new(
            angle.cos() * 1.4,
            1.2 + (index % 5) as f32 * 0.18,
            angle.sin() * 1.4,
        );

        commands.spawn((
            Mesh3d(particle_mesh.clone()),
            MeshMaterial3d(materials.particle.clone()),
            Transform::from_translation(center + offset),
            BreakParticle {
                velocity,
                lifetime: Timer::from_seconds(0.45, TimerMode::Once),
            },
        ));
    }
}

fn spawn_dropped_block(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    block: Block,
    position: Vec3,
    velocity: Vec3,
    pickup_delay: f32,
) {
    commands.spawn((
        Mesh3d(meshes.add(build_item_mesh(block))),
        MeshMaterial3d(materials.terrain.clone()),
        Transform::from_translation(position).with_scale(Vec3::splat(0.32)),
        DroppedBlock {
            block,
            velocity,
            age: 0.0,
            pickup_delay,
            mass: block.mass(),
        },
    ));
}

fn update_dropped_blocks(
    time: Res<Time>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut items: Query<(&mut DroppedBlock, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);

    for (mut item, mut transform) in &mut items {
        item.age += dt;
        item.velocity.y -= 14.0 * dt;
        transform.translation += item.velocity * dt;
        transform.rotate_y((1.4 / item.mass.max(0.1)) * dt);

        let below = (transform.translation + Vec3::new(0.0, -0.22, 0.0))
            .floor()
            .as_ivec3();

        if item.velocity.y < 0.0 && is_solid_at(below, &chunks) {
            transform.translation.y = below.y as f32 + 1.22;
            let mass = item.mass.max(0.4);
            item.velocity *= Vec3::new(0.42, -0.12, 0.42) / mass;
            if item.velocity.length_squared() < 0.08 {
                item.velocity = Vec3::ZERO;
            }
        }
    }
}

fn pickup_dropped_blocks(
    mut commands: Commands,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    mut inventory: ResMut<Inventory>,
    mut pickup_events: MessageWriter<ItemPickedUp>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    items: Query<(Entity, &DroppedBlock, &Transform)>,
) {
    if is_open(&settings_state) || chat_open(&chat_state) {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    for (entity, item, transform) in &items {
        if item.age < item.pickup_delay {
            continue;
        }

        let offset = transform.translation - camera.translation;
        let horizontal = Vec2::new(offset.x, offset.z).length();

        if horizontal <= PICKUP_RADIUS && offset.y.abs() <= 2.2 && inventory.add(item.block) {
            pickup_events.write(ItemPickedUp { block: item.block });
            commands.entity(entity).despawn();
        }
    }
}

fn dropped_position(block: IVec3) -> Vec3 {
    let offset = Vec3::new(
        random_unit(block, 11) * 0.34 - 0.17,
        0.16,
        random_unit(block, 29) * 0.34 - 0.17,
    );
    block.as_vec3() + Vec3::splat(0.5) + offset
}

fn dropped_velocity(block: IVec3) -> Vec3 {
    Vec3::new(
        random_unit(block, 41) * 1.1 - 0.55,
        1.4 + random_unit(block, 53) * 0.8,
        random_unit(block, 67) * 1.1 - 0.55,
    )
}

fn random_unit(block: IVec3, salt: i32) -> f32 {
    let value = block.x.wrapping_mul(73_856_093)
        ^ block.y.wrapping_mul(19_349_663)
        ^ block.z.wrapping_mul(83_492_791)
        ^ salt;
    let mixed = value.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (mixed as u32 % 10_000) as f32 / 10_000.0
}

fn update_break_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut BreakParticle, &mut Transform)>,
) {
    let dt = time.delta_secs();

    for (entity, mut particle, mut transform) in &mut particles {
        particle.lifetime.tick(time.delta());
        particle.velocity.y -= 9.8 * dt;
        transform.translation += particle.velocity * dt;
        transform.scale *= 1.0 - (3.2 * dt).min(0.85);

        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn update_block_selection(
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    break_state: Res<BreakState>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    if is_open(&settings_state) || chat_open(&chat_state) {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let Some(hit) = raycast_blocks(camera.translation, *camera.forward(), &chunks) else {
        return;
    };

    let block = block_at_world(hit.block, &chunks);
    let break_time = block.hardness().max(0.1);
    let progress = if break_state.target == Some(hit.block) {
        (break_state.progress / break_time).clamp(0.0, 1.0)
    } else {
        0.0
    };

    gizmos.cube(
        Transform::from_translation(hit.block.as_vec3() + Vec3::splat(0.5))
            .with_scale(Vec3::splat(1.015)),
        Color::srgba(0.02, 0.02, 0.02, 0.95),
    );

    if progress > 0.0 {
        draw_block_cracks(&mut gizmos, hit.block, hit.normal, progress);
    }
}

fn update_physics_debug(
    debug: Option<Res<PhysicsDebug>>,
    trees: Query<(&FallingTree, &Transform)>,
    items: Query<(&DroppedBlock, &Transform)>,
    mut gizmos: Gizmos,
) {
    let Some(debug) = debug else {
        return;
    };
    if !debug.enabled {
        return;
    }

    for (tree, _) in &trees {
        let axis = tree_axis(tree);
        let start = tree.base;
        let end = tree.base + axis * tree.height;
        let center = start.lerp(end, 0.5);

        gizmos.line(start, end, Color::srgb(1.0, 0.62, 0.15));
        gizmos.cube(
            Transform::from_translation(center)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis))
                .with_scale(Vec3::new(0.92, tree.height, 0.92)),
            Color::srgba(1.0, 0.62, 0.15, 0.8),
        );
        gizmos.cube(
            Transform::from_translation(start).with_scale(Vec3::splat(0.18)),
            Color::srgb(0.1, 0.9, 0.25),
        );
        gizmos.cube(
            Transform::from_translation(end).with_scale(Vec3::splat(0.18)),
            Color::srgb(0.9, 0.1, 0.1),
        );
    }

    for (item, transform) in &items {
        gizmos.cube(
            Transform::from_translation(transform.translation).with_scale(Vec3::splat(0.64)),
            Color::srgba(0.2, 0.65, 1.0, 0.85),
        );
        gizmos.cube(
            Transform::from_translation(transform.translation)
                .with_scale(Vec3::splat(PICKUP_RADIUS * 2.0)),
            Color::srgba(0.2, 1.0, 0.45, 0.28),
        );
        if item.velocity.length_squared() > 0.01 {
            gizmos.line(
                transform.translation,
                transform.translation + item.velocity.normalize() * 1.2,
                Color::srgb(1.0, 1.0, 0.2),
            );
        }
    }
}

fn draw_block_cracks(gizmos: &mut Gizmos, block: IVec3, normal: IVec3, progress: f32) {
    let stage = (progress * 7.0).ceil() as usize;
    let lines = [
        (Vec2::new(-0.28, 0.0), Vec2::new(0.0, 0.0)),
        (Vec2::new(0.0, 0.0), Vec2::new(0.22, 0.18)),
        (Vec2::new(0.0, 0.0), Vec2::new(0.18, -0.22)),
        (Vec2::new(-0.04, 0.02), Vec2::new(-0.2, 0.25)),
        (Vec2::new(-0.02, -0.02), Vec2::new(-0.22, -0.2)),
        (Vec2::new(0.14, 0.14), Vec2::new(0.34, 0.3)),
        (Vec2::new(0.12, -0.16), Vec2::new(0.32, -0.31)),
    ];

    for (from, to) in lines.into_iter().take(stage.min(lines.len())) {
        gizmos.line(
            crack_point(block, normal, from),
            crack_point(block, normal, to),
            Color::srgba(0.0, 0.0, 0.0, 0.92),
        );
    }
}

fn crack_point(block: IVec3, normal: IVec3, point: Vec2) -> Vec3 {
    let center = block.as_vec3() + Vec3::splat(0.5);
    let push = normal.as_vec3() * 0.512;

    if normal.x != 0 {
        center + push + Vec3::new(0.0, point.x, point.y)
    } else if normal.y != 0 {
        center + push + Vec3::new(point.x, 0.0, point.y)
    } else {
        center + push + Vec3::new(point.x, point.y, 0.0)
    }
}

fn raycast_blocks(
    origin: Vec3,
    direction: Vec3,
    chunks: &Query<(&Chunk, &GlobalTransform)>,
) -> Option<BlockHit> {
    voxel_raycast(origin, direction, BLOCK_REACH, |block_pos| {
        block_at_world(block_pos, chunks).is_solid()
    })
}

fn raycast_blocks_mut(
    origin: Vec3,
    direction: Vec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Option<BlockHit> {
    voxel_raycast(origin, direction, BLOCK_REACH, |block_pos| {
        for (chunk, transform, _) in chunks.iter() {
            let local = block_pos - transform.translation().floor().as_ivec3();
            if Chunk::contains(local) {
                return chunk.get(local.x, local.y, local.z).is_solid();
            }
        }
        false
    })
}

pub fn is_solid_at(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> bool {
    block_at_world(world_pos, chunks).is_solid()
}

fn block_at_world(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> Block {
    for (chunk, transform) in chunks.iter() {
        let local = world_pos - transform.translation().floor().as_ivec3();
        if Chunk::contains(local) {
            return chunk.get(local.x, local.y, local.z);
        }
    }
    Block::Air
}

fn block_at_world_mut(
    world_pos: IVec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Block {
    for (chunk, transform, _) in chunks.iter() {
        let local = world_pos - transform.translation().floor().as_ivec3();
        if Chunk::contains(local) {
            return chunk.get(local.x, local.y, local.z);
        }
    }
    Block::Air
}

fn connected_log_count(
    world_pos: IVec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> i32 {
    let mut bottom = world_pos.y;
    let mut top = world_pos.y;

    while block_at_world_mut(IVec3::new(world_pos.x, bottom - 1, world_pos.z), chunks) == Block::Log
    {
        bottom -= 1;
    }
    while block_at_world_mut(IVec3::new(world_pos.x, top + 1, world_pos.z), chunks) == Block::Log {
        top += 1;
    }

    top - bottom + 1
}

fn connected_log_span_in_chunk(chunk: &Chunk, world_pos: IVec3, local_y: i32) -> (i32, i32) {
    let mut bottom = local_y;
    let mut top = local_y;
    let local_x = world_pos.x.rem_euclid(CHUNK_SIZE as i32);
    let local_z = world_pos.z.rem_euclid(CHUNK_SIZE as i32);

    while bottom > 0 && chunk.get(local_x, bottom - 1, local_z) == Block::Log {
        bottom -= 1;
    }

    while top < CHUNK_HEIGHT as i32 - 1 && chunk.get(local_x, top + 1, local_z) == Block::Log {
        top += 1;
    }

    (
        world_pos.y - (local_y - bottom),
        world_pos.y + (top - local_y),
    )
}

fn set_block_at_world(
    world_pos: IVec3,
    block: Block,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    meshes: &mut Assets<Mesh>,
) -> bool {
    let mut changed = None;

    for (mut chunk, transform, _) in chunks.iter_mut() {
        let origin = transform.translation().floor().as_ivec3();
        let local = world_pos - origin;

        if !Chunk::contains(local) {
            continue;
        }

        chunk.set_local(local, block);
        changed = Some((origin, local));
        break;
    }

    let Some((origin, local)) = changed else {
        return false;
    };

    rebuild_changed_chunks(origin, local, chunks, meshes);
    true
}

fn world_chunk_coord(world_pos: IVec3) -> IVec2 {
    let size = CHUNK_SIZE as i32;
    IVec2::new(world_pos.x.div_euclid(size), world_pos.z.div_euclid(size))
}

fn chunk_origin(coord: IVec2) -> IVec3 {
    IVec3::new(coord.x * CHUNK_SIZE as i32, 0, coord.y * CHUNK_SIZE as i32)
}

fn empty_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn should_rebuild_chunk(changed_origin: IVec3, changed_local: IVec3, chunk_origin: IVec3) -> bool {
    if chunk_origin == changed_origin {
        return true;
    }

    let size = CHUNK_SIZE as i32;

    (changed_local.x == 0 && chunk_origin == changed_origin + IVec3::new(-size, 0, 0))
        || (changed_local.x == size - 1 && chunk_origin == changed_origin + IVec3::new(size, 0, 0))
        || (changed_local.z == 0 && chunk_origin == changed_origin + IVec3::new(0, 0, -size))
        || (changed_local.z == size - 1 && chunk_origin == changed_origin + IVec3::new(0, 0, size))
}

fn rebuild_changed_chunks(
    origin: IVec3,
    local: IVec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    meshes: &mut Assets<Mesh>,
) {
    let snapshot = chunk_snapshot(chunks);

    for (chunk, transform, mut mesh_handle) in chunks {
        let chunk_origin = transform.translation().floor().as_ivec3();

        if should_rebuild_chunk(origin, local, chunk_origin) {
            let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
                block_from_snapshot(chunk_origin + local, &snapshot)
            })
            .unwrap_or_else(empty_mesh);
            *mesh_handle = Mesh3d(meshes.add(mesh));
        }
    }
}

fn rebuild_all_chunks(
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    meshes: &mut Assets<Mesh>,
) {
    let snapshot = chunk_snapshot(chunks);

    for (chunk, transform, mut mesh_handle) in chunks {
        let chunk_origin = transform.translation().floor().as_ivec3();
        let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
            block_from_snapshot(chunk_origin + local, &snapshot)
        })
        .unwrap_or_else(empty_mesh);
        *mesh_handle = Mesh3d(meshes.add(mesh));
    }
}

fn chunk_snapshot(
    chunks: &Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Vec<(IVec3, Chunk)> {
    chunks
        .iter()
        .map(|(chunk, transform, _)| (transform.translation().floor().as_ivec3(), chunk.clone()))
        .collect()
}

fn chunk_snapshot_immut(
    chunks: &Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Vec<(IVec3, Chunk)> {
    chunks
        .iter()
        .map(|(chunk, transform, _)| (transform.translation().floor().as_ivec3(), chunk.clone()))
        .collect()
}

fn block_from_snapshot(world_pos: IVec3, chunks: &[(IVec3, Chunk)]) -> Block {
    for (origin, chunk) in chunks {
        let local = world_pos - *origin;
        if Chunk::contains(local) {
            return chunk.get(local.x, local.y, local.z);
        }
    }
    Block::Air
}

#[derive(Clone, Copy)]
struct BlockHit {
    block: IVec3,
    normal: IVec3,
}

fn voxel_raycast(
    origin: Vec3,
    direction: Vec3,
    reach: f32,
    mut is_solid: impl FnMut(IVec3) -> bool,
) -> Option<BlockHit> {
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
    let mut traveled = 0.0;
    let mut normal = IVec3::ZERO;

    while traveled <= reach {
        if is_solid(block) {
            return Some(BlockHit { block, normal });
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
