use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::PrimitiveTopology};

use crate::game::{
    audio::optional_sound,
    camera::{PlayerCamera, PlayerController, player_intersects_block},
    events::{
        BlockBroken, BlockDamaged, BlockPlaced, ItemDropped, ItemPickedUp, PlayerDamaged,
        PlayerDied,
    },
    inventory::Inventory,
    settings::{SettingsState, is_open},
};

mod block;
mod chunk;
mod generation;
mod materials;
mod meshing;

pub use block::Block;
pub use chunk::Chunk;
pub use materials::BlockMaterials;
pub use meshing::build_item_mesh;

use chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use generation::generate_chunk;
use meshing::build_chunk_mesh_with_neighbors;

pub struct WorldPlugin;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct ChunkCoord(IVec2);

#[derive(Resource)]
struct WorldStreaming {
    radius: i32,
    unload_radius: i32,
    timer: Timer,
}

#[derive(Resource)]
struct BlockAudio {
    break_sound: Option<Handle<AudioSource>>,
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

#[derive(Resource, Default)]
struct BreakState {
    target: Option<IVec3>,
    progress: f32,
}

#[derive(Resource)]
struct FallingBlockScan {
    timer: Timer,
}

const BLOCK_REACH: f32 = 7.0;
const PICKUP_RADIUS: f32 = 1.45;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BreakState::default())
            .insert_resource(WorldStreaming {
                radius: 5,
                unload_radius: 7,
                timer: Timer::from_seconds(0.35, TimerMode::Repeating),
            })
            .insert_resource(FallingBlockScan {
                timer: Timer::from_seconds(0.12, TimerMode::Repeating),
            })
            .add_systems(
                Startup,
                (materials::setup_materials, setup_block_audio, spawn_world).chain(),
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
                    drop_inventory_on_death,
                    update_dropped_blocks,
                    pickup_dropped_blocks,
                    update_break_particles,
                    update_block_selection,
                )
                    .chain(),
            );
    }
}

fn setup_block_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(BlockAudio {
        break_sound: optional_sound(&asset_server, "sounds/block_break_dirt.ogg"),
    });
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    let radius = 5;
    let mut generated_chunks = Vec::new();

    for chunk_x in -radius..=radius {
        for chunk_z in -radius..=radius {
            let origin = IVec3::new(chunk_x * CHUNK_SIZE as i32, 0, chunk_z * CHUNK_SIZE as i32);

            generated_chunks.push((origin, generate_chunk(IVec2::new(chunk_x, chunk_z))));
        }
    }

    for (origin, chunk) in &generated_chunks {
        let Some(mesh) = build_chunk_mesh_with_neighbors(&chunk, |local| {
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
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut chunks: Query<(Entity, &ChunkCoord, &Chunk, &GlobalTransform, &mut Mesh3d)>,
) {
    streaming.timer.tick(time.delta());

    if !streaming.timer.just_finished() {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let player_chunk = world_chunk_coord(camera.translation.floor().as_ivec3());
    let mut snapshot: Vec<(IVec3, Chunk)> = chunks
        .iter()
        .map(|(_, _, chunk, transform, _)| {
            (transform.translation().floor().as_ivec3(), chunk.clone())
        })
        .collect();
    let loaded: Vec<IVec2> = chunks.iter().map(|(_, coord, _, _, _)| coord.0).collect();
    let mut generated = Vec::new();

    for x in player_chunk.x - streaming.radius..=player_chunk.x + streaming.radius {
        for z in player_chunk.y - streaming.radius..=player_chunk.y + streaming.radius {
            let coord = IVec2::new(x, z);
            if loaded.contains(&coord) {
                continue;
            }

            let origin = chunk_origin(coord);
            let chunk = generate_chunk(coord);
            snapshot.push((origin, chunk.clone()));
            generated.push((coord, origin, chunk));
        }
    }

    for (coord, origin, chunk) in generated {
        let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
            block_from_snapshot(origin + local, &snapshot)
        })
        .unwrap_or_else(empty_mesh);
        spawn_chunk_entity(&mut commands, &mut meshes, &materials, origin, chunk, mesh)
            .insert(ChunkCoord(coord));
    }

    for (entity, coord, _, _, _) in &mut chunks {
        let distance = (coord.0 - player_chunk).abs();
        if distance.x > streaming.unload_radius || distance.y > streaming.unload_radius {
            commands.entity(entity).despawn();
        }
    }

    if !snapshot.is_empty() {
        for (_, _, chunk, transform, mut mesh_handle) in &mut chunks {
            let origin = transform.translation().floor().as_ivec3();
            let mesh = build_chunk_mesh_with_neighbors(chunk, |local| {
                block_from_snapshot(origin + local, &snapshot)
            })
            .unwrap_or_else(empty_mesh);
            *mesh_handle = Mesh3d(meshes.add(mesh));
        }
    }
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

fn break_selected_block(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    settings_state: Res<SettingsState>,
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
    if is_open(&settings_state) || !mouse.pressed(MouseButton::Left) {
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
    let break_time = hit_block.hardness().max(0.1);
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

    for (mut chunk, transform, _) in &mut chunks {
        let local = hit.block - transform.translation().floor().as_ivec3();

        if !is_inside_chunk(local) {
            continue;
        }

        let block = chunk.get(local.x, local.y, local.z);
        if !block.is_solid() {
            continue;
        }

        chunk.set_local(local, Block::Air);
        changed_origin = Some(transform.translation().floor().as_ivec3());
        changed_local = Some(local);
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

    rebuild_changed_chunks(origin, local, &mut chunks, &mut meshes);
}

fn place_selected_block(
    mouse: Res<ButtonInput<MouseButton>>,
    settings_state: Res<SettingsState>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    players: Query<&PlayerController, With<PlayerCamera>>,
    mut inventory: ResMut<Inventory>,
    mut placed_events: MessageWriter<BlockPlaced>,
    mut chunks: Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if is_open(&settings_state) || !mouse.just_pressed(MouseButton::Right) {
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
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut inventory: ResMut<Inventory>,
    mut dropped_events: MessageWriter<ItemDropped>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
) {
    if is_open(&settings_state) || !keys.just_pressed(KeyCode::KeyQ) {
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
    let snapshot = chunk_snapshot(&chunks);
    let mut falling = Vec::new();

    for (chunk, transform, _) in chunks.iter() {
        let origin = transform.translation().floor().as_ivec3();
        for y in 1..CHUNK_HEIGHT as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                for x in 0..CHUNK_SIZE as i32 {
                    let local = IVec3::new(x, y, z);
                    let world = origin + local;

                    if (world.x - center.x).abs() > 18 || (world.z - center.z).abs() > 18 {
                        continue;
                    }

                    let block = chunk.get(x, y, z);
                    if !block.falls()
                        || block_from_snapshot(world + IVec3::NEG_Y, &snapshot).is_solid()
                    {
                        continue;
                    }

                    falling.push((world, block));
                    if falling.len() >= 16 {
                        break;
                    }
                }
                if falling.len() >= 16 {
                    break;
                }
            }
            if falling.len() >= 16 {
                break;
            }
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
    mut inventory: ResMut<Inventory>,
    mut pickup_events: MessageWriter<ItemPickedUp>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    items: Query<(Entity, &DroppedBlock, &Transform)>,
) {
    if is_open(&settings_state) {
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
    let value = block.x * 73_856_093 ^ block.y * 19_349_663 ^ block.z * 83_492_791 ^ salt;
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
    break_state: Res<BreakState>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    chunks: Query<(&Chunk, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    if is_open(&settings_state) {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let Some(hit) = raycast_blocks(camera.translation, *camera.forward(), &chunks) else {
        return;
    };

    let block = block_at(hit.block, &chunks);
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
        block_at(block_pos, chunks).is_solid()
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

            if chunk.get(local.x, local.y, local.z).is_solid() {
                return true;
            }
        }

        false
    })
}

fn block_at(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> Block {
    block_at_world(world_pos, chunks)
}

pub fn is_solid_at(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> bool {
    block_at_world(world_pos, chunks).is_solid()
}

fn block_at_world(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> Block {
    for (chunk, transform) in chunks.iter() {
        let chunk_origin = transform.translation().floor().as_ivec3();
        let local = world_pos - chunk_origin;
        let block = chunk.get(local.x, local.y, local.z);

        if block.is_solid() {
            return block;
        }
    }

    Block::Air
}

fn block_at_world_mut(
    world_pos: IVec3,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
) -> Block {
    for (chunk, transform, _) in chunks.iter() {
        let chunk_origin = transform.translation().floor().as_ivec3();
        let local = world_pos - chunk_origin;
        let block = chunk.get(local.x, local.y, local.z);

        if block.is_solid() {
            return block;
        }
    }

    Block::Air
}

fn set_block_at_world(
    world_pos: IVec3,
    block: Block,
    chunks: &mut Query<(&mut Chunk, &GlobalTransform, &mut Mesh3d)>,
    meshes: &mut Assets<Mesh>,
) -> bool {
    let mut changed_origin = None;
    let mut changed_local = None;

    for (mut chunk, transform, _) in chunks.iter_mut() {
        let origin = transform.translation().floor().as_ivec3();
        let local = world_pos - origin;

        if !is_inside_chunk(local) {
            continue;
        }

        chunk.set_local(local, block);
        changed_origin = Some(origin);
        changed_local = Some(local);
        break;
    }

    let Some(origin) = changed_origin else {
        return false;
    };
    let Some(local) = changed_local else {
        return false;
    };

    rebuild_changed_chunks(origin, local, chunks, meshes);
    true
}

fn is_inside_chunk(local: IVec3) -> bool {
    local.x >= 0
        && local.y >= 0
        && local.z >= 0
        && local.x < CHUNK_SIZE as i32
        && local.y < CHUNK_HEIGHT as i32
        && local.z < CHUNK_SIZE as i32
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

    let chunk_size = CHUNK_SIZE as i32;

    (changed_local.x == 0 && chunk_origin == changed_origin + IVec3::new(-chunk_size, 0, 0))
        || (changed_local.x == chunk_size - 1
            && chunk_origin == changed_origin + IVec3::new(chunk_size, 0, 0))
        || (changed_local.z == 0 && chunk_origin == changed_origin + IVec3::new(0, 0, -chunk_size))
        || (changed_local.z == chunk_size - 1
            && chunk_origin == changed_origin + IVec3::new(0, 0, chunk_size))
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

fn chunk_snapshot(
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

        if is_inside_chunk(local) {
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
