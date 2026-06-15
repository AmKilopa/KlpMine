use bevy::prelude::*;

use crate::game::world::Block;

pub struct GameEventsPlugin;

#[derive(Resource, Default)]
pub struct GameplayStats {
    pub broken_blocks: u64,
    pub placed_blocks: u64,
    pub dropped_items: u64,
    pub picked_items: u64,
    pub last_block_position: Option<IVec3>,
    pub last_block_mass: f32,
}

#[derive(Message, Clone, Copy)]
pub struct BlockBroken {
    pub block: Block,
    pub position: IVec3,
}

#[derive(Message, Clone, Copy)]
pub struct BlockDamaged {
    pub block: Block,
    pub position: IVec3,
    pub progress: f32,
}

#[derive(Message, Clone, Copy)]
pub struct BlockPlaced {
    pub block: Block,
    pub position: IVec3,
}

#[derive(Message, Clone, Copy)]
pub struct ItemDropped {
    pub block: Block,
    pub position: Vec3,
}

#[derive(Message, Clone, Copy)]
pub struct ItemPickedUp {
    pub block: Block,
}

#[derive(Message, Clone, Copy)]
pub struct PlayerDamaged {
    pub amount: f32,
}

#[derive(Message, Clone, Copy)]
pub struct PlayerDied;

#[derive(Message, Clone, Copy)]
pub struct PlayerRespawned;

impl Plugin for GameEventsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameplayStats::default())
            .add_message::<BlockBroken>()
            .add_message::<BlockDamaged>()
            .add_message::<BlockPlaced>()
            .add_message::<ItemDropped>()
            .add_message::<ItemPickedUp>()
            .add_message::<PlayerDamaged>()
            .add_message::<PlayerDied>()
            .add_message::<PlayerRespawned>()
            .add_systems(Update, update_gameplay_stats);
    }
}

fn update_gameplay_stats(
    mut stats: ResMut<GameplayStats>,
    mut broken: MessageReader<BlockBroken>,
    mut damaged: MessageReader<BlockDamaged>,
    mut placed: MessageReader<BlockPlaced>,
    mut dropped: MessageReader<ItemDropped>,
    mut picked: MessageReader<ItemPickedUp>,
) {
    for event in broken.read() {
        stats.broken_blocks += 1;
        stats.last_block_position = Some(event.position);
        stats.last_block_mass = event.block.mass();
    }

    for event in damaged.read() {
        stats.last_block_position = Some(event.position);
        stats.last_block_mass = event.block.mass();
        let _ = event.progress;
    }

    for event in placed.read() {
        stats.placed_blocks += 1;
        stats.last_block_position = Some(event.position);
        stats.last_block_mass = event.block.mass();
    }

    for event in dropped.read() {
        stats.dropped_items += 1;
        stats.last_block_position = Some(event.position.floor().as_ivec3());
        stats.last_block_mass = event.block.mass();
    }

    for event in picked.read() {
        stats.picked_items += 1;
        stats.last_block_mass = event.block.mass();
    }
}
