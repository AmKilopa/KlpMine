use std::collections::HashSet;

use bevy::{
    asset::RenderAssetUsages,
    ecs::{query::QueryFilter, system::SystemParam},
    light::NotShadowCaster,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use bevy_rapier3d::prelude::*;

use crate::game::{
    audio::{effect_playback, optional_sound},
    camera::{PlayerCamera, PlayerController, player_intersects_block},
    chat::{ChatState, is_open as chat_open},
    debug::PhysicsDebug,
    events::{
        BlockBroken, BlockDamaged, BlockPlaced, ItemDropped, ItemPickedUp, PlayerDamaged,
        PlayerDied,
    },
    health::PlayerHealth,
    inventory::Inventory,
    settings::{GameSettings, SettingsState, is_open},
};

mod block;
mod chunk;
mod collisions;
mod fluid;
mod generation;
mod materials;
mod meshing;
mod physics;

pub use block::Block;
pub use chunk::Chunk;
pub use fluid::WaterSimulation;
pub use materials::BlockMaterials;
pub use meshing::{build_item_mesh, build_log_stack_mesh};

use chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use collisions::build_chunk_collider_with_neighbors;
pub use generation::{WorldSeed, player_spawn_position};
use generation::{generate_chunk, new_world_seed};
use meshing::{
    build_chunk_mesh_with_neighbors, build_chunk_water_mesh_with_neighbors, build_tree_shadow_mesh,
};
use physics::{
    PLAYER_MAX_LIFT_MASS, TREE_DRAG_BREAK_DISTANCE, TREE_DRAG_GRAB_DISTANCE,
    TREE_DRAG_STRETCH_LIMIT, TREE_GRIP_RAY_RADIUS, TREE_RADIUS, clamp_dropped_item_velocity,
    falling_tree_mass, move_dropped_item,
};

pub struct WorldPlugin;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct ChunkCoord(IVec2);

#[derive(Component)]
struct WaterChunkMesh;

#[derive(Component)]
struct ShadowChunkMesh;

#[derive(Resource)]
struct WorldStreaming {
    chunks_per_tick: usize,
    timer: Timer,
}

#[derive(Resource)]
struct BlockAudio {
    break_sound: Option<Handle<AudioSource>>,
    leaf_sound: Option<Handle<AudioSource>>,
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
pub struct FallingTree {
    height: f32,
    mass: f32,
    drag_anchor: Option<Vec3>,
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

#[derive(Resource, Default)]
struct ChunkLoadStatus {
    pending: usize,
    phase: usize,
    visible_timer: f32,
}

#[derive(Component)]
struct ChunkLoadPanel;

#[derive(Component)]
struct ChunkLoadBar(usize);

#[derive(Component)]
struct TreeGripMarker;

#[derive(Component)]
struct PhysicsLabel {
    target: Entity,
}

#[derive(Resource)]
struct WorldDebugLog {
    water_timer: Timer,
}

type EditableChunkQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Chunk,
        &'static GlobalTransform,
        &'static mut Mesh3d,
        &'static mut Collider,
    ),
    Without<WaterChunkMesh>,
>;
type WaterMeshQuery<'w, 's> =
    Query<'w, 's, (&'static ChunkCoord, &'static mut Mesh3d), With<WaterChunkMesh>>;
type PhysicsLabelQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PhysicsLabel,
        &'static mut Text2d,
        &'static mut Transform,
    ),
    (
        Without<FallingTree>,
        Without<FallingBlock>,
        Without<DroppedBlock>,
        Without<PlayerCamera>,
    ),
>;

#[derive(SystemParam)]
struct WaterFlowParams<'w, 's> {
    water: ResMut<'w, WaterSimulation>,
    debug: Option<Res<'w, PhysicsDebug>>,
    debug_log: ResMut<'w, WorldDebugLog>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    chunks: EditableChunkQuery<'w, 's>,
    water_meshes: WaterMeshQuery<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

#[derive(SystemParam)]
struct PhysicsDebugParams<'w, 's> {
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    trees: Query<'w, 's, (Entity, &'static FallingTree, &'static Transform)>,
    falling_blocks: Query<'w, 's, (Entity, &'static FallingBlock, &'static Transform)>,
    items: Query<'w, 's, (Entity, &'static DroppedBlock, &'static Transform)>,
    labels: PhysicsLabelQuery<'w, 's>,
}

struct ChunkSpawnParts {
    mesh: Mesh,
    water_mesh: Mesh,
    collider: Collider,
}

struct TreeSpawnAssets<'a> {
    meshes: &'a mut Assets<Mesh>,
    materials: &'a BlockMaterials,
    audio: &'a BlockAudio,
    settings: &'a GameSettings,
}

#[derive(SystemParam)]
struct StreamWorldParams<'w, 's> {
    materials: Res<'w, BlockMaterials>,
    meshes: ResMut<'w, Assets<Mesh>>,
    streaming: ResMut<'w, WorldStreaming>,
    status: ResMut<'w, ChunkLoadStatus>,
    settings: Res<'w, GameSettings>,
    seed: Res<'w, WorldSeed>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    chunks: Query<
        'w,
        's,
        (
            Entity,
            &'static ChunkCoord,
            &'static Chunk,
            &'static GlobalTransform,
        ),
    >,
    water_chunks: Query<'w, 's, (Entity, &'static ChunkCoord), With<WaterChunkMesh>>,
}

#[derive(SystemParam)]
struct BreakBlockParams<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    time: Res<'w, Time>,
    settings_state: Res<'w, SettingsState>,
    chat_state: Res<'w, ChatState>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    audio: Res<'w, BlockAudio>,
    settings: Res<'w, GameSettings>,
    break_state: ResMut<'w, BreakState>,
    broken_events: MessageWriter<'w, BlockBroken>,
    damaged_events: MessageWriter<'w, BlockDamaged>,
    dropped_events: MessageWriter<'w, ItemDropped>,
    chunks: EditableChunkQuery<'w, 's>,
    water_meshes: WaterMeshQuery<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: Res<'w, BlockMaterials>,
}

#[derive(SystemParam)]
struct PlaceBlockParams<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    settings_state: Res<'w, SettingsState>,
    chat_state: Res<'w, ChatState>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    players: Query<'w, 's, &'static PlayerController, With<PlayerCamera>>,
    trees: Query<'w, 's, (&'static FallingTree, &'static Transform)>,
    inventory: ResMut<'w, Inventory>,
    placed_events: MessageWriter<'w, BlockPlaced>,
    chunks: EditableChunkQuery<'w, 's>,
    water_meshes: WaterMeshQuery<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    water: ResMut<'w, WaterSimulation>,
}

#[derive(SystemParam)]
struct DropBlockParams<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    settings_state: Res<'w, SettingsState>,
    chat_state: Res<'w, ChatState>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    inventory: ResMut<'w, Inventory>,
    dropped_events: MessageWriter<'w, ItemDropped>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: Res<'w, BlockMaterials>,
}

#[derive(SystemParam)]
struct FallingBlockScanParams<'w, 's> {
    time: Res<'w, Time>,
    scan: ResMut<'w, FallingBlockScan>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    chunks: EditableChunkQuery<'w, 's>,
    water_meshes: WaterMeshQuery<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: Res<'w, BlockMaterials>,
}

#[derive(SystemParam)]
struct FallingBlockUpdateParams<'w, 's> {
    time: Res<'w, Time>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    damage_events: MessageWriter<'w, PlayerDamaged>,
    falling_blocks: Query<
        'w,
        's,
        (Entity, &'static mut FallingBlock, &'static mut Transform),
        Without<PlayerCamera>,
    >,
    chunks: EditableChunkQuery<'w, 's>,
    water_meshes: WaterMeshQuery<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

#[derive(SystemParam)]
struct PickupDroppedParams<'w, 's> {
    settings_state: Res<'w, SettingsState>,
    chat_state: Res<'w, ChatState>,
    health: Res<'w, PlayerHealth>,
    inventory: ResMut<'w, Inventory>,
    pickup_events: MessageWriter<'w, ItemPickedUp>,
    cameras: Query<'w, 's, &'static Transform, With<PlayerCamera>>,
    items: Query<'w, 's, (Entity, &'static DroppedBlock, &'static Transform)>,
}

const BLOCK_REACH: f32 = 7.0;
const PICKUP_RADIUS: f32 = 1.45;
const PLAYER_PUSH_FORCE: f32 = 34.0;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BreakState::default())
            .insert_resource(new_world_seed())
            .insert_resource(WaterSimulation::new())
            .insert_resource(WorldStreaming {
                chunks_per_tick: 1,
                timer: Timer::from_seconds(0.12, TimerMode::Repeating),
            })
            .insert_resource(FallingBlockScan {
                timer: Timer::from_seconds(0.35, TimerMode::Repeating),
            })
            .insert_resource(WorldDebugLog {
                water_timer: Timer::from_seconds(0.75, TimerMode::Repeating),
            })
            .insert_resource(ChunkLoadStatus::default())
            .add_systems(
                Startup,
                (
                    materials::setup_materials,
                    setup_block_audio,
                    spawn_world,
                    spawn_chunk_load_ui,
                    spawn_tree_grip_marker,
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
                    flow_water,
                    update_falling_blocks,
                    update_falling_trees,
                    update_tree_grip_marker,
                    drop_inventory_on_death,
                    update_dropped_blocks,
                    pickup_dropped_blocks,
                    update_break_particles,
                    update_block_selection,
                    update_physics_debug,
                    update_shadow_visibility,
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
    });
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<BlockMaterials>,
    settings: Res<GameSettings>,
    seed: Res<WorldSeed>,
) {
    let radius = 2;
    let mut generated_chunks = Vec::new();

    info!("world seed: {}", seed.value);

    for chunk_x in -radius..=radius {
        for chunk_z in -radius..=radius {
            let coord = IVec2::new(chunk_x, chunk_z);
            if !chunk_in_render_distance(coord, IVec2::ZERO, radius) {
                continue;
            }
            let origin = IVec3::new(chunk_x * CHUNK_SIZE as i32, 0, chunk_z * CHUNK_SIZE as i32);
            generated_chunks.push((origin, generate_chunk(coord, seed.value)));
        }
    }

    for (origin, chunk) in &generated_chunks {
        let mesh = build_chunk_mesh_with_neighbors(chunk, |local| {
            block_from_chunk_or_snapshot(chunk, *origin, local, &generated_chunks)
        })
        .unwrap_or_else(empty_mesh);
        let water_mesh = build_chunk_water_mesh_with_neighbors(
            chunk,
            |local| block_from_chunk_or_snapshot(chunk, *origin, local, &generated_chunks),
            |_| 1.0,
        )
        .unwrap_or_else(empty_mesh);
        let collider = build_chunk_collider_with_neighbors(chunk, |local| {
            block_from_chunk_or_snapshot(chunk, *origin, local, &generated_chunks)
        });

        spawn_chunk_entity(
            &mut commands,
            &mut meshes,
            &materials,
            *origin,
            chunk.clone(),
            ChunkSpawnParts {
                mesh,
                water_mesh,
                collider,
            },
            settings.shadows,
        );
    }
}

fn stream_world_chunks(mut commands: Commands, time: Res<Time>, params: StreamWorldParams) {
    let StreamWorldParams {
        materials,
        mut meshes,
        mut streaming,
        mut status,
        settings,
        seed,
        cameras,
        chunks,
        water_chunks,
    } = params;

    streaming.timer.tick(time.delta());

    if !streaming.timer.just_finished() {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let player_chunk = world_chunk_coord(camera.translation.floor().as_ivec3());
    let render_distance = settings.render_distance.clamp(2, 7);
    let unload_distance = render_distance;
    let forward = Vec2::new(camera.forward().x, camera.forward().z).normalize_or_zero();
    let loaded: HashSet<IVec2> = chunks.iter().map(|(_, coord, _, _)| coord.0).collect();
    let mut missing = Vec::new();

    for x in player_chunk.x - render_distance..=player_chunk.x + render_distance {
        for z in player_chunk.y - render_distance..=player_chunk.y + render_distance {
            let coord = IVec2::new(x, z);
            if loaded.contains(&coord)
                || !chunk_should_load(coord, player_chunk, forward, render_distance)
            {
                continue;
            }
            missing.push(coord);
        }
    }

    for (entity, coord, _, _) in &chunks {
        if !chunk_should_keep(coord.0, player_chunk, forward, unload_distance) {
            commands.entity(entity).despawn();
        }
    }

    for (entity, coord) in &water_chunks {
        if !chunk_should_keep(coord.0, player_chunk, forward, unload_distance) {
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
            let chunk = generate_chunk(coord, seed.value);
            snapshot.push((origin, chunk.clone()));
            (coord, origin, chunk)
        })
        .collect();

    for (_, origin, chunk) in generated {
        let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
            block_from_chunk_or_snapshot(&chunk, origin, local, &snapshot)
        })
        .unwrap_or_else(empty_mesh);
        let water_mesh = build_chunk_water_mesh_with_neighbors(
            &chunk,
            |local| block_from_chunk_or_snapshot(&chunk, origin, local, &snapshot),
            |_| 1.0,
        )
        .unwrap_or_else(empty_mesh);
        let collider = build_chunk_collider_with_neighbors(&chunk, |local| {
            block_from_chunk_or_snapshot(&chunk, origin, local, &snapshot)
        });
        spawn_chunk_entity(
            &mut commands,
            &mut meshes,
            &materials,
            origin,
            chunk,
            ChunkSpawnParts {
                mesh,
                water_mesh,
                collider,
            },
            settings.shadows,
        );
    }
}

fn spawn_chunk_load_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(18),
                bottom: px(18),
                width: px(58),
                height: px(18),
                align_items: AlignItems::End,
                column_gap: px(5),
                ..default()
            },
            GlobalZIndex(i32::MAX - 6),
            Visibility::Hidden,
            ChunkLoadPanel,
        ))
        .with_children(|parent| {
            for index in 0..4 {
                parent.spawn((
                    Node {
                        width: px(10),
                        height: px(8),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.82, 0.92, 1.0, 0.55)),
                    ChunkLoadBar(index),
                ));
            }
        });
}

fn spawn_tree_grip_marker(
    mut commands: Commands,
    materials: Res<BlockMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.12))),
        MeshMaterial3d(materials.debug_marker.clone()),
        Transform::from_xyz(0.0, -200.0, 0.0),
        Visibility::Hidden,
        TreeGripMarker,
        NotShadowCaster,
    ));
}

fn update_tree_grip_marker(
    debug: Option<Res<PhysicsDebug>>,
    trees: Query<(&FallingTree, &Transform), Without<TreeGripMarker>>,
    mut markers: Query<(&mut Transform, &mut Visibility), With<TreeGripMarker>>,
) {
    let Ok((mut transform, mut visibility)) = markers.single_mut() else {
        return;
    };

    if !debug.as_ref().is_some_and(|debug| debug.enabled) {
        *visibility = Visibility::Hidden;
        return;
    }

    if let Some(anchor) = active_tree_drag_anchor(&trees) {
        transform.translation = anchor;
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}

fn update_chunk_load_ui(
    time: Res<Time>,
    mut status: ResMut<ChunkLoadStatus>,
    mut panels: Query<&mut Visibility, With<ChunkLoadPanel>>,
    mut bars: Query<(&ChunkLoadBar, &mut Node, &mut BackgroundColor)>,
) {
    status.visible_timer = (status.visible_timer - time.delta_secs()).max(0.0);
    let Ok(mut visibility) = panels.single_mut() else {
        return;
    };

    if status.visible_timer <= 0.0 || status.pending == 0 {
        *visibility = Visibility::Hidden;
        return;
    }

    *visibility = Visibility::Visible;
    for (bar, mut node, mut color) in &mut bars {
        let active = bar.0 <= status.phase;
        node.height = px(if active { 18.0 } else { 8.0 });
        *color = BackgroundColor(if active {
            Color::srgba(0.95, 0.98, 1.0, 0.92)
        } else {
            Color::srgba(0.5, 0.68, 0.9, 0.46)
        });
    }
}

fn update_shadow_visibility(
    settings: Res<GameSettings>,
    mut shadows: Query<&mut Visibility, With<ShadowChunkMesh>>,
) {
    if !settings.is_changed() {
        return;
    }

    let visibility = if settings.shadows {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    for mut shadow in &mut shadows {
        *shadow = visibility;
    }
}

fn spawn_chunk_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &BlockMaterials,
    origin: IVec3,
    chunk: Chunk,
    parts: ChunkSpawnParts,
    shadows_visible: bool,
) {
    let coord = world_chunk_coord(origin);
    let shadow_mesh = build_tree_shadow_mesh(&chunk);

    commands.spawn((
        Mesh3d(meshes.add(parts.water_mesh)),
        MeshMaterial3d(materials.water.clone()),
        Transform::from_translation(origin.as_vec3()),
        ChunkCoord(coord),
        WaterChunkMesh,
    ));

    let mut terrain = commands.spawn((
        Mesh3d(meshes.add(parts.mesh)),
        MeshMaterial3d(materials.terrain.clone()),
        Transform::from_translation(origin.as_vec3()),
        RigidBody::Fixed,
        parts.collider,
        Friction::coefficient(0.92),
        Restitution::coefficient(0.02),
        ChunkCoord(coord),
        chunk,
    ));

    if let Some(shadow_mesh) = shadow_mesh {
        terrain.with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(shadow_mesh)),
                MeshMaterial3d(materials.shadow.clone()),
                Transform::default(),
                if shadows_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
                ShadowChunkMesh,
                NotShadowCaster,
            ));
        });
    }
}

fn chunk_should_load(
    coord: IVec2,
    player_chunk: IVec2,
    forward: Vec2,
    render_distance: i32,
) -> bool {
    let _ = forward;
    chunk_in_render_distance(coord, player_chunk, render_distance)
}

fn chunk_should_keep(
    coord: IVec2,
    player_chunk: IVec2,
    forward: Vec2,
    render_distance: i32,
) -> bool {
    let _ = forward;
    chunk_in_render_distance(coord, player_chunk, render_distance)
}

fn chunk_in_render_distance(coord: IVec2, player_chunk: IVec2, render_distance: i32) -> bool {
    let offset = coord - player_chunk;
    let radius_sq = render_distance.max(0) * render_distance.max(0);
    offset.x * offset.x + offset.y * offset.y <= radius_sq
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

fn break_selected_block(mut commands: Commands, params: BreakBlockParams) {
    let BreakBlockParams {
        mouse,
        time,
        settings_state,
        chat_state,
        cameras,
        audio,
        settings,
        mut break_state,
        mut broken_events,
        mut damaged_events,
        mut dropped_events,
        mut chunks,
        mut water_meshes,
        mut meshes,
        materials,
    } = params;

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

    for (mut chunk, transform, _, _) in &mut chunks {
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
            if let Some(drop) = block.drop_item() {
                let position = dropped_position(hit.block);
                spawn_dropped_block(
                    &mut commands,
                    &mut meshes,
                    &materials,
                    drop,
                    position,
                    dropped_velocity(hit.block),
                    0.25,
                );
                dropped_events.write(ItemDropped {
                    block: drop,
                    position,
                });
            }
        }

        spawn_break_effect(
            &mut commands,
            &mut meshes,
            &materials,
            hit.block.as_vec3() + Vec3::splat(0.5),
        );

        if let Some(sound) = &audio.break_sound {
            commands.spawn((AudioPlayer::new(sound.clone()), effect_playback(&settings)));
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
        let rebuild_min = IVec3::new(position.x - 5, span.0 - 1, position.z - 5);
        let rebuild_max = IVec3::new(position.x + 5, span.1 + 5, position.z + 5);
        let mut tree_assets = TreeSpawnAssets {
            meshes: &mut meshes,
            materials: &materials,
            audio: &audio,
            settings: &settings,
        };
        spawn_falling_tree_from_block(
            &mut commands,
            &mut chunks,
            &mut tree_assets,
            position,
            span,
            camera.translation,
        );
        rebuild_area_chunks(
            rebuild_min,
            rebuild_max,
            &mut chunks,
            &mut water_meshes,
            &mut meshes,
            true,
            None,
        );
        return;
    }

    rebuild_changed_chunks(
        origin,
        local,
        &mut chunks,
        &mut water_meshes,
        &mut meshes,
        None,
    );
}

fn place_selected_block(params: PlaceBlockParams) {
    let PlaceBlockParams {
        mouse,
        settings_state,
        chat_state,
        cameras,
        players,
        trees,
        mut inventory,
        mut placed_events,
        mut chunks,
        mut water_meshes,
        mut meshes,
        mut water,
    } = params;

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
    if camera_targets_tree(camera, &trees) {
        return;
    }
    let Some(hit) = raycast_blocks_mut(camera.translation, *camera.forward(), &mut chunks) else {
        return;
    };

    if hit.normal == IVec3::ZERO {
        return;
    }

    let target = hit.block + hit.normal;
    if block_at_world_mut(target, &mut chunks).is_solid()
        || player_intersects_block(target, player)
        || tree_blocks_position(target, &trees)
    {
        return;
    }

    if set_block_at_world(target, block, &mut chunks, &mut water_meshes, &mut meshes) {
        water.clear(target);
        let below = target + IVec3::NEG_Y;
        if block_at_world_mut(below, &mut chunks) == Block::Grass {
            set_block_at_world(
                below,
                Block::Dirt,
                &mut chunks,
                &mut water_meshes,
                &mut meshes,
            );
        }
        inventory.remove_selected();
        placed_events.write(BlockPlaced {
            block,
            position: target,
        });
    }
}

fn drop_selected_block(mut commands: Commands, params: DropBlockParams) {
    let DropBlockParams {
        keys,
        settings_state,
        chat_state,
        cameras,
        mut inventory,
        mut dropped_events,
        mut meshes,
        materials,
    } = params;

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
        forward * 3.0 + Vec3::Y * 1.1,
        0.75,
    );
    dropped_events.write(ItemDropped { block, position });
}

fn start_falling_blocks(mut commands: Commands, params: FallingBlockScanParams) {
    let FallingBlockScanParams {
        time,
        mut scan,
        cameras,
        mut chunks,
        mut water_meshes,
        mut meshes,
        materials,
    } = params;

    scan.timer.tick(time.delta());

    if !scan.timer.just_finished() {
        return;
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    let center = camera.translation.floor().as_ivec3();
    let mut falling = Vec::new();
    let radius = 8;
    let snapshot = chunk_snapshot_near_immut(&chunks, center, radius + 2);
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
        if set_block_at_world(
            position,
            Block::Air,
            &mut chunks,
            &mut water_meshes,
            &mut meshes,
        ) {
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

fn update_falling_blocks(mut commands: Commands, params: FallingBlockUpdateParams) {
    let FallingBlockUpdateParams {
        time,
        cameras,
        mut damage_events,
        mut falling_blocks,
        mut chunks,
        mut water_meshes,
        mut meshes,
    } = params;

    let dt = time.delta_secs().min(0.05);
    let player = cameras.single().ok();

    for (entity, mut falling, mut transform) in &mut falling_blocks {
        falling.velocity.y = (falling.velocity.y - 22.0 * dt).max(-38.0);
        transform.translation += falling.velocity * dt;
        transform.rotate_y(0.9 * dt);

        if !falling.damaged_player
            && let Some(player_transform) = player
            && falling_hits_player(transform.translation, player_transform.translation)
        {
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

            if set_block_at_world(
                target,
                falling.block,
                &mut chunks,
                &mut water_meshes,
                &mut meshes,
            ) {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn flow_water(time: Res<Time>, mut flow: WaterFlowParams) {
    flow.debug_log.water_timer.tick(time.delta());

    if !flow.water.tick(time.delta()) {
        return;
    }

    let Ok(camera) = flow.cameras.single() else {
        return;
    };

    let center = camera.translation.floor().as_ivec3();
    let snapshot = chunk_snapshot_near_immut(&flow.chunks, center, 12);
    let changes = flow.water.simulate(center, &snapshot);

    if !changes.is_empty() {
        apply_block_changes(
            &changes,
            &mut flow.chunks,
            &mut flow.water_meshes,
            &mut flow.meshes,
            &flow.water,
        );
    }

    if flow.debug.as_ref().is_some_and(|debug| debug.enabled)
        && flow.debug_log.water_timer.just_finished()
    {
        let stats = flow.water.debug_stats();
        info!(
            "water: cells={} sources={} visible={} mass={:.2} changes={}",
            stats.active_cells,
            stats.source_cells,
            stats.visible_cells,
            stats.total_mass,
            stats.last_changes
        );
    }
}

fn apply_block_changes(
    changes: &[(IVec3, Block)],
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
    water_meshes: &mut Query<(&ChunkCoord, &mut Mesh3d), With<WaterChunkMesh>>,
    meshes: &mut Assets<Mesh>,
    water: &WaterSimulation,
) {
    let mut min = IVec3::splat(i32::MAX);
    let mut max = IVec3::splat(i32::MIN);

    for (pos, block) in changes {
        set_block_direct(*pos, *block, chunks);
        min = min.min(*pos);
        max = max.max(*pos);
    }

    rebuild_area_chunks(
        min - IVec3::ONE,
        max + IVec3::ONE,
        chunks,
        water_meshes,
        meshes,
        false,
        Some(water),
    );
}

fn spawn_falling_tree_from_block(
    commands: &mut Commands,
    chunks: &mut EditableChunkQuery,
    assets: &mut TreeSpawnAssets,
    broken: IVec3,
    span: (i32, i32),
    player_position: Vec3,
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
                        &mut *assets.meshes,
                        assets.materials,
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
    let mass = falling_tree_mass(height);
    let tilt = Quat::from_rotation_arc(Vec3::Y, (Vec3::Y + fall_dir * 0.08).normalize());
    let angular_axis = Vec3::Y.cross(fall_dir).normalize_or_zero();

    commands.spawn((
        Mesh3d(assets.meshes.add(build_log_stack_mesh(height))),
        MeshMaterial3d(assets.materials.terrain.clone()),
        Transform::from_translation(tree_center).with_rotation(tilt),
        RigidBody::Dynamic,
        Collider::cuboid(TREE_RADIUS, height as f32 * 0.5, TREE_RADIUS),
        ColliderMassProperties::Mass(mass),
        Velocity {
            linear: fall_dir * 0.35,
            angular: angular_axis * 1.45,
        },
        Damping {
            linear_damping: 0.18,
            angular_damping: 0.36,
        },
        ExternalImpulse::default(),
        Ccd::enabled(),
        FallingTree {
            height: height as f32,
            mass,
            drag_anchor: None,
            damaged_player: false,
        },
    ));

    if let Some(sound) = &assets.audio.leaf_sound {
        commands.spawn((
            AudioPlayer::new(sound.clone()),
            effect_playback(assets.settings),
        ));
    }
}

fn update_falling_trees(
    _time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    cameras: Query<(&Transform, &PlayerController), With<PlayerCamera>>,
    mut damage_events: MessageWriter<PlayerDamaged>,
    mut trees: Query<
        (
            &mut FallingTree,
            &Transform,
            &mut ExternalImpulse,
            &Velocity,
        ),
        Without<PlayerCamera>,
    >,
) {
    for (mut tree, transform, mut impulse, velocity) in &mut trees {
        if let Ok((player_transform, player)) = cameras.single() {
            if !tree.damaged_player
                && let Some(damage) =
                    tree_impact_damage(&tree, transform, velocity, player_transform.translation)
            {
                damage_events.write(PlayerDamaged { amount: damage });
                tree.damaged_player = true;
                info!(
                    "tree impact: mass={:.2} speed={:.2} damage={:.2}",
                    tree.mass,
                    tree_impact_speed(&tree, velocity),
                    damage
                );
            }

            push_tree_from_player(&tree, transform, player, &mut impulse);

            if mouse.just_pressed(MouseButton::Right) {
                let hit_point = tree_ray_hit_point(
                    &tree,
                    transform,
                    player_transform.translation,
                    *player_transform.forward(),
                );

                if let Some(point) = hit_point {
                    let close_enough =
                        point.distance(player_transform.translation) < TREE_DRAG_GRAB_DISTANCE;

                    if close_enough {
                        tree.drag_anchor =
                            Some(transform.rotation.inverse() * (point - transform.translation));
                        info!(
                            "tree grip: mass={:.2} height={:.2} anchor={:.2},{:.2},{:.2}",
                            tree.mass, tree.height, point.x, point.y, point.z
                        );
                    }
                }
            }

            if !mouse.pressed(MouseButton::Right) {
                tree.drag_anchor = None;
            }

            if let Some(anchor) = tree.drag_anchor {
                let anchor_world = transform.translation + transform.rotation * anchor;
                let target = player_transform.translation
                    + *player_transform.forward() * 2.25
                    + Vec3::new(0.0, -1.0, 0.0);
                let pull = target - anchor_world;
                let distance = pull.length();

                if anchor_world.distance(player_transform.translation) > TREE_DRAG_BREAK_DISTANCE
                    || distance > TREE_DRAG_STRETCH_LIMIT
                {
                    info!(
                        "tree grip released: mass={:.2} distance={:.2}",
                        tree.mass, distance
                    );
                    tree.drag_anchor = None;
                    continue;
                }

                if distance > 0.2 {
                    let current_speed = Vec2::new(velocity.linear.x, velocity.linear.z).length();
                    let speed_factor = (0.75 - current_speed).max(0.0);
                    let mut pull = pull.normalize_or_zero();
                    if tree.mass > PLAYER_MAX_LIFT_MASS {
                        pull.y = pull.y.min(0.0);
                        pull = pull.normalize_or_zero();
                    }
                    let strength =
                        (speed_factor * 8.0 + distance.min(2.4) * 18.0) / tree.mass.max(1.0);
                    *impulse += ExternalImpulse::at_point(
                        pull * strength,
                        anchor_world,
                        transform.translation,
                    );
                }
            }
        }
    }
}

fn push_tree_from_player(
    tree: &FallingTree,
    transform: &Transform,
    player: &PlayerController,
    impulse: &mut ExternalImpulse,
) {
    let player_velocity = player.horizontal_velocity();
    let speed = Vec2::new(player_velocity.x, player_velocity.z).length();
    if speed < 1.8 {
        return;
    }

    let (player_min, player_max) = player_body_aabb(player.feet_position());
    if !tree_intersects_aabb(tree, transform, player_min, player_max) {
        return;
    }

    let direction = Vec3::new(player_velocity.x, 0.0, player_velocity.z).normalize_or_zero();
    if direction == Vec3::ZERO {
        return;
    }

    let contact = closest_tree_point_to_point(tree, transform, (player_min + player_max) * 0.5);
    let mass_resistance = (tree.mass / 90.0).max(1.0);
    let strength = (speed - 1.8).min(4.0) * PLAYER_PUSH_FORCE / mass_resistance;
    *impulse += ExternalImpulse::at_point(direction * strength, contact, transform.translation);
}

fn tree_impact_damage(
    tree: &FallingTree,
    transform: &Transform,
    velocity: &Velocity,
    eye_position: Vec3,
) -> Option<f32> {
    let (player_min, player_max) = player_aabb_from_eye(eye_position);
    if !tree_intersects_aabb(tree, transform, player_min, player_max) {
        return None;
    }

    let player_center = (player_min + player_max) * 0.5;
    let contact = closest_tree_point_to_point(tree, transform, player_center);
    let overhead = contact.y - (player_min.y + 0.45);
    if overhead <= 0.0 {
        return None;
    }

    let speed = tree_impact_speed(tree, velocity);
    if speed < 1.2 {
        return None;
    }

    let mass_factor = (tree.mass / 40.0).sqrt().clamp(0.8, 2.5);
    let damage = ((speed - 0.8) * mass_factor * 0.75 + overhead.min(1.5) * 0.8).min(10.0);
    (damage >= 0.5).then(|| (damage * 2.0).round() / 2.0)
}

fn tree_impact_speed(tree: &FallingTree, velocity: &Velocity) -> f32 {
    let angular_tip_speed = velocity.angular.length() * tree.height.min(7.0) * 0.35;
    velocity.linear.length() + angular_tip_speed + (-velocity.linear.y).max(0.0) * 0.8
}

fn tree_axis(transform: &Transform) -> Vec3 {
    (transform.rotation * Vec3::Y).normalize_or(Vec3::Y)
}

fn closest_tree_point_to_point(tree: &FallingTree, transform: &Transform, point: Vec3) -> Vec3 {
    let axis = tree_axis(transform);
    let start = transform.translation - axis * (tree.height * 0.5);
    let along = (point - start).dot(axis).clamp(0.0, tree.height);
    start + axis * along
}

pub fn tree_blocks_position(
    block: IVec3,
    trees: &Query<(&FallingTree, &Transform), impl QueryFilter>,
) -> bool {
    let min = block.as_vec3();
    let max = min + Vec3::ONE;
    trees
        .iter()
        .any(|(tree, transform)| tree_intersects_aabb(tree, transform, min, max))
}

pub fn tree_collides_player(
    position: Vec3,
    trees: &Query<(&FallingTree, &Transform), impl QueryFilter>,
) -> bool {
    let (min, max) = player_body_aabb(position);

    trees
        .iter()
        .any(|(tree, transform)| tree_intersects_aabb(tree, transform, min, max))
}

pub fn tree_supports_player(
    position: Vec3,
    trees: &Query<(&FallingTree, &Transform), impl QueryFilter>,
) -> bool {
    let min = Vec3::new(position.x - 0.26, position.y - 0.08, position.z - 0.26);
    let max = Vec3::new(position.x + 0.26, position.y + 0.04, position.z + 0.26);

    trees
        .iter()
        .any(|(tree, transform)| tree_intersects_aabb(tree, transform, min, max))
}

pub fn active_tree_drag_anchor(
    trees: &Query<(&FallingTree, &Transform), impl QueryFilter>,
) -> Option<Vec3> {
    trees.iter().find_map(|(tree, transform)| {
        tree.drag_anchor
            .map(|anchor| transform.translation + transform.rotation * anchor)
    })
}

fn tree_intersects_aabb(tree: &FallingTree, transform: &Transform, min: Vec3, max: Vec3) -> bool {
    let axis = tree_axis(transform);
    let samples = ((tree.height * 4.0).ceil() as i32).clamp(12, 48);
    let expanded_min = min - Vec3::splat(TREE_RADIUS);
    let expanded_max = max + Vec3::splat(TREE_RADIUS);
    let start = transform.translation - axis * (tree.height * 0.5);
    let end = transform.translation + axis * (tree.height * 0.5);

    let segment_min = start.min(end) - Vec3::splat(TREE_RADIUS);
    let segment_max = start.max(end) + Vec3::splat(TREE_RADIUS);
    if segment_min.x > max.x
        || segment_max.x < min.x
        || segment_min.y > max.y
        || segment_max.y < min.y
        || segment_min.z > max.z
        || segment_max.z < min.z
    {
        return false;
    }

    for index in 0..=samples {
        let t = index as f32 / samples as f32;
        let point = start + axis * (tree.height * t);
        if point.x >= expanded_min.x
            && point.x <= expanded_max.x
            && point.y >= expanded_min.y
            && point.y <= expanded_max.y
            && point.z >= expanded_min.z
            && point.z <= expanded_max.z
        {
            return true;
        }
    }

    false
}

fn camera_targets_tree(camera: &Transform, trees: &Query<(&FallingTree, &Transform)>) -> bool {
    trees.iter().any(|(tree, transform)| {
        ray_hits_tree(tree, transform, camera.translation, *camera.forward())
    })
}

fn ray_hits_tree(tree: &FallingTree, transform: &Transform, origin: Vec3, direction: Vec3) -> bool {
    tree_ray_hit_point(tree, transform, origin, direction).is_some()
}

fn tree_ray_hit_point(
    tree: &FallingTree,
    transform: &Transform,
    origin: Vec3,
    direction: Vec3,
) -> Option<Vec3> {
    let direction = direction.normalize_or_zero();
    if direction == Vec3::ZERO {
        return None;
    }

    let axis = tree_axis(transform);
    let samples = ((tree.height * 8.0).ceil() as i32).clamp(16, 80);
    let mut best = None;
    let mut best_along = f32::MAX;

    for index in 0..=samples {
        let t = index as f32 / samples as f32;
        let point = transform.translation - axis * (tree.height * 0.5) + axis * (tree.height * t);
        let to_point = point - origin;
        let along = to_point.dot(direction);

        if !(0.0..=BLOCK_REACH).contains(&along) {
            continue;
        }

        let closest = origin + direction * along;
        let distance = closest.distance(point);
        if distance <= TREE_GRIP_RAY_RADIUS {
            let entry_offset =
                (TREE_GRIP_RAY_RADIUS * TREE_GRIP_RAY_RADIUS - distance * distance).sqrt();
            let entry_along = (along - entry_offset).max(0.0);

            if entry_along < best_along {
                best_along = entry_along;
                best = Some(origin + direction * entry_along);
            }
        }
    }

    best
}

fn set_block_direct(
    world_pos: IVec3,
    block: Block,
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
) {
    for (mut chunk, transform, _, _) in chunks.iter_mut() {
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
    let (player_min, player_max) = player_aabb_from_eye(eye_position);
    let block_min = block_center - Vec3::splat(0.5);
    let block_max = block_center + Vec3::splat(0.5);

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

fn player_aabb_from_eye(eye_position: Vec3) -> (Vec3, Vec3) {
    player_body_aabb(eye_position - Vec3::Y * 1.62)
}

fn player_body_aabb(position: Vec3) -> (Vec3, Vec3) {
    let min = Vec3::new(position.x - 0.34, position.y + 0.001, position.z - 0.34);
    let max = Vec3::new(
        position.x + 0.34,
        position.y + 1.8 - 0.001,
        position.z + 0.34,
    );
    (min, max)
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
            velocity: clamp_dropped_item_velocity(velocity),
            age: 0.0,
            pickup_delay,
            mass: block.mass().max(0.12),
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
        item.velocity.y = (item.velocity.y - 14.0 * dt).max(-18.0);
        move_dropped_item(
            &mut transform.translation,
            &mut item.velocity,
            dt,
            |block| is_solid_at(block, &chunks),
        );
        transform.rotate_y((0.9 / item.mass.sqrt().max(0.35)) * dt);
    }
}

fn pickup_dropped_blocks(mut commands: Commands, params: PickupDroppedParams) {
    let PickupDroppedParams {
        settings_state,
        chat_state,
        health,
        mut inventory,
        mut pickup_events,
        cameras,
        items,
    } = params;

    if health.dead || is_open(&settings_state) || chat_open(&chat_state) {
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
    mut commands: Commands,
    debug: Option<Res<PhysicsDebug>>,
    mut physics: PhysicsDebugParams,
    mut gizmos: Gizmos,
) {
    let Some(debug) = debug else {
        return;
    };
    if !debug.enabled {
        for (entity, _, _, _) in &mut physics.labels {
            commands.entity(entity).despawn();
        }
        return;
    }

    let camera = physics.cameras.single().ok();

    if let Some(camera) = camera {
        let start = camera.translation;
        let end = start + *camera.forward() * BLOCK_REACH;
        gizmos.line(start, end, Color::srgb(0.2, 0.95, 1.0));
        gizmos.cube(
            Transform::from_translation(end).with_scale(Vec3::splat(0.12)),
            Color::srgb(0.2, 0.95, 1.0),
        );
    }

    let mut active_labels = HashSet::new();

    for (entity, tree, transform) in &physics.trees {
        let axis = tree_axis(transform);
        let start = transform.translation - axis * (tree.height * 0.5);
        let end = transform.translation + axis * (tree.height * 0.5);
        let anchor_world = tree
            .drag_anchor
            .map(|anchor| transform.translation + transform.rotation * anchor);

        gizmos.line(start, end, Color::srgb(1.0, 0.62, 0.15));
        gizmos.cube(
            Transform::from_translation(transform.translation)
                .with_rotation(transform.rotation)
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
        if let Some(anchor) = anchor_world {
            gizmos.cube(
                Transform::from_translation(anchor).with_scale(Vec3::splat(0.22)),
                Color::srgb(1.0, 0.02, 0.02),
            );
        }
        if let Some(camera) = camera {
            let text = if let Some(anchor) = anchor_world {
                format!(
                    "tree\nmass {:.2}\nheight {:.1}\ngrip {:.1}m",
                    tree.mass,
                    tree.height,
                    anchor.distance(camera.translation)
                )
            } else {
                format!("tree\nmass {:.2}\nheight {:.1}", tree.mass, tree.height)
            };
            update_debug_label(
                &mut commands,
                &mut physics.labels,
                &mut active_labels,
                entity,
                text,
                end + Vec3::Y * 0.75,
                camera.translation,
            );
        }
    }

    for (entity, block, transform) in &physics.falling_blocks {
        gizmos.cube(
            Transform::from_translation(transform.translation).with_scale(Vec3::splat(1.0)),
            Color::srgba(1.0, 0.85, 0.15, 0.85),
        );
        if block.velocity.length_squared() > 0.01 {
            gizmos.line(
                transform.translation,
                transform.translation + block.velocity * 0.18,
                Color::srgb(1.0, 0.2, 0.1),
            );
        }
        if let Some(camera) = camera {
            update_debug_label(
                &mut commands,
                &mut physics.labels,
                &mut active_labels,
                entity,
                format!(
                    "{:?}\nmass {:.2}\nvy {:.1}",
                    block.block,
                    block.block.mass(),
                    block.velocity.y
                ),
                transform.translation + Vec3::Y * 1.0,
                camera.translation,
            );
        }
    }

    for (entity, item, transform) in &physics.items {
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
        if let Some(camera) = camera {
            update_debug_label(
                &mut commands,
                &mut physics.labels,
                &mut active_labels,
                entity,
                format!("{:?}\nmass {:.2}", item.block, item.mass),
                transform.translation + Vec3::Y * 0.6,
                camera.translation,
            );
        }
    }

    for (entity, label, _, _) in &mut physics.labels {
        if !active_labels.contains(&label.target) {
            commands.entity(entity).despawn();
        }
    }
}

fn update_debug_label(
    commands: &mut Commands,
    labels: &mut PhysicsLabelQuery,
    active: &mut HashSet<Entity>,
    target: Entity,
    content: String,
    position: Vec3,
    camera_position: Vec3,
) {
    active.insert(target);

    for (_, label, mut text, mut transform) in labels.iter_mut() {
        if label.target != target {
            continue;
        }

        text.0 = content;
        transform.translation = position;
        transform.look_at(camera_position, Vec3::Y);
        return;
    }

    let mut transform = Transform::from_translation(position);
    transform.look_at(camera_position, Vec3::Y);
    commands.spawn((
        Text2d::new(content),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.96, 0.72)),
        TextLayout::new_with_justify(Justify::Center),
        transform,
        PhysicsLabel { target },
        NotShadowCaster,
    ));
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
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
) -> Option<BlockHit> {
    voxel_raycast(origin, direction, BLOCK_REACH, |block_pos| {
        for (chunk, transform, _, _) in chunks.iter() {
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

pub fn block_at_position(world_pos: IVec3, chunks: &Query<(&Chunk, &GlobalTransform)>) -> Block {
    block_at_world(world_pos, chunks)
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
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
) -> Block {
    for (chunk, transform, _, _) in chunks.iter() {
        let local = world_pos - transform.translation().floor().as_ivec3();
        if Chunk::contains(local) {
            return chunk.get(local.x, local.y, local.z);
        }
    }
    Block::Air
}

fn connected_log_count(
    world_pos: IVec3,
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
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
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
    water_meshes: &mut Query<(&ChunkCoord, &mut Mesh3d), With<WaterChunkMesh>>,
    meshes: &mut Assets<Mesh>,
) -> bool {
    let mut changed = None;

    for (mut chunk, transform, _, _) in chunks.iter_mut() {
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

    rebuild_changed_chunks(origin, local, chunks, water_meshes, meshes, None);
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
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
    water_meshes: &mut Query<(&ChunkCoord, &mut Mesh3d), With<WaterChunkMesh>>,
    meshes: &mut Assets<Mesh>,
    water: Option<&WaterSimulation>,
) {
    let snapshot = chunk_snapshot(chunks);

    for (chunk, transform, mut mesh_handle, mut collider) in chunks {
        let chunk_origin = transform.translation().floor().as_ivec3();

        if should_rebuild_chunk(origin, local, chunk_origin) {
            let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
                block_from_chunk_or_snapshot(&chunk, chunk_origin, local, &snapshot)
            })
            .unwrap_or_else(empty_mesh);
            let rebuilt_collider = build_chunk_collider_with_neighbors(&chunk, |local| {
                block_from_chunk_or_snapshot(&chunk, chunk_origin, local, &snapshot)
            });
            *mesh_handle = Mesh3d(meshes.add(mesh));
            *collider = rebuilt_collider;
            rebuild_water_chunk_mesh(chunk_origin, &chunk, &snapshot, water_meshes, meshes, water);
        }
    }
}

fn rebuild_area_chunks(
    min: IVec3,
    max: IVec3,
    chunks: &mut Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
    water_meshes: &mut Query<(&ChunkCoord, &mut Mesh3d), With<WaterChunkMesh>>,
    meshes: &mut Assets<Mesh>,
    rebuild_colliders: bool,
    water: Option<&WaterSimulation>,
) {
    let snapshot = chunk_snapshot(chunks);

    for (chunk, transform, mut mesh_handle, mut collider) in chunks {
        let chunk_origin = transform.translation().floor().as_ivec3();
        let chunk_max = chunk_origin
            + IVec3::new(
                CHUNK_SIZE as i32 - 1,
                CHUNK_HEIGHT as i32 - 1,
                CHUNK_SIZE as i32 - 1,
            );

        if chunk_origin.x > max.x
            || chunk_max.x < min.x
            || chunk_origin.y > max.y
            || chunk_max.y < min.y
            || chunk_origin.z > max.z
            || chunk_max.z < min.z
        {
            continue;
        }

        let mesh = build_chunk_mesh_with_neighbors(&chunk, |local| {
            block_from_chunk_or_snapshot(&chunk, chunk_origin, local, &snapshot)
        })
        .unwrap_or_else(empty_mesh);
        *mesh_handle = Mesh3d(meshes.add(mesh));
        if rebuild_colliders {
            *collider = build_chunk_collider_with_neighbors(&chunk, |local| {
                block_from_chunk_or_snapshot(&chunk, chunk_origin, local, &snapshot)
            });
        }
        rebuild_water_chunk_mesh(chunk_origin, &chunk, &snapshot, water_meshes, meshes, water);
    }
}

fn rebuild_water_chunk_mesh(
    chunk_origin: IVec3,
    chunk: &Chunk,
    snapshot: &[(IVec3, Chunk)],
    water_meshes: &mut Query<(&ChunkCoord, &mut Mesh3d), With<WaterChunkMesh>>,
    meshes: &mut Assets<Mesh>,
    water: Option<&WaterSimulation>,
) {
    let coord = world_chunk_coord(chunk_origin);
    let mesh = build_chunk_water_mesh_with_neighbors(
        chunk,
        |local| block_from_chunk_or_snapshot(chunk, chunk_origin, local, snapshot),
        |local| {
            let world_pos = chunk_origin + local;
            water
                .map(|water| {
                    water.fill_fraction_for_block(
                        world_pos,
                        block_from_chunk_or_snapshot(chunk, chunk_origin, local, snapshot),
                    )
                })
                .unwrap_or(1.0)
        },
    )
    .unwrap_or_else(empty_mesh);

    for (water_coord, mut mesh_handle) in water_meshes {
        if water_coord.0 == coord {
            *mesh_handle = Mesh3d(meshes.add(mesh));
            return;
        }
    }
}

fn chunk_snapshot(
    chunks: &Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
) -> Vec<(IVec3, Chunk)> {
    chunks
        .iter()
        .map(|(chunk, transform, _, _)| (transform.translation().floor().as_ivec3(), chunk.clone()))
        .collect()
}

fn chunk_snapshot_near_immut(
    chunks: &Query<
        (&mut Chunk, &GlobalTransform, &mut Mesh3d, &mut Collider),
        Without<WaterChunkMesh>,
    >,
    center: IVec3,
    radius: i32,
) -> Vec<(IVec3, Chunk)> {
    let min = center - IVec3::new(radius, CHUNK_HEIGHT as i32, radius);
    let max = center + IVec3::new(radius, CHUNK_HEIGHT as i32, radius);

    chunks
        .iter()
        .filter_map(|(chunk, transform, _, _)| {
            let origin = transform.translation().floor().as_ivec3();
            let chunk_max = origin
                + IVec3::new(
                    CHUNK_SIZE as i32 - 1,
                    CHUNK_HEIGHT as i32 - 1,
                    CHUNK_SIZE as i32 - 1,
                );

            if origin.x > max.x || chunk_max.x < min.x || origin.z > max.z || chunk_max.z < min.z {
                None
            } else {
                Some((origin, chunk.clone()))
            }
        })
        .collect()
}

fn block_from_chunk_or_snapshot(
    chunk: &Chunk,
    chunk_origin: IVec3,
    local: IVec3,
    chunks: &[(IVec3, Chunk)],
) -> Block {
    if Chunk::contains(local) {
        return chunk.get(local.x, local.y, local.z);
    }

    block_from_snapshot(chunk_origin + local, chunks)
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
