use std::time::Duration;

use bevy::prelude::*;

use super::{block::Block, chunk::Chunk};

#[derive(Resource)]
pub struct WaterSimulation {
    timer: Timer,
}

#[derive(Clone, Copy, Default)]
pub struct WaterDebugStats {
    pub last_changes: usize,
}

const WATER_TICK_SECONDS: f32 = 0.12;

impl WaterSimulation {
    pub fn new() -> Self {
        Self {
            timer: Timer::from_seconds(WATER_TICK_SECONDS, TimerMode::Repeating),
        }
    }

    pub fn tick(&mut self, delta: Duration) -> bool {
        self.timer.tick(delta);
        self.timer.just_finished()
    }

    pub fn clear(&mut self, _pos: IVec3) {}

    pub fn debug_stats(&self) -> WaterDebugStats {
        WaterDebugStats { last_changes: 0 }
    }

    pub fn fill_fraction_for_block(&self, _pos: IVec3, block: Block) -> f32 {
        if block == Block::Water { 1.0 } else { 0.0 }
    }

    pub fn simulate(&mut self, _center: IVec3, _chunks: &[(IVec3, Chunk)]) -> Vec<(IVec3, Block)> {
        vec![]
    }
}
