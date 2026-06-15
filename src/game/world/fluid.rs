use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use bevy::prelude::*;

use super::{
    block::Block,
    chunk::{CHUNK_HEIGHT, CHUNK_SIZE, Chunk},
};

#[derive(Clone, Copy)]
struct WaterCell {
    mass: f32,
}

#[derive(Resource)]
pub struct WaterSimulation {
    timer: Timer,
    cells: HashMap<IVec3, WaterCell>,
}

const WATER_TICK_SECONDS: f32 = 0.08;
const WATER_RADIUS: i32 = 14;
const WATER_Y_RANGE: i32 = 11;
const WATER_FULL_MASS: f32 = 1.0;
const WATER_MIN_MASS: f32 = 0.015;
const WATER_VISIBLE_MASS: f32 = 0.08;
const WATER_DOWN_FLOW: f32 = 0.72;
const WATER_SIDE_FLOW: f32 = 0.24;

impl WaterSimulation {
    pub fn new() -> Self {
        Self {
            timer: Timer::from_seconds(WATER_TICK_SECONDS, TimerMode::Repeating),
            cells: HashMap::new(),
        }
    }

    pub fn tick(&mut self, delta: Duration) -> bool {
        self.timer.tick(delta);
        self.timer.just_finished()
    }

    pub fn clear(&mut self, pos: IVec3) {
        self.cells.remove(&pos);
    }

    pub fn simulate(&mut self, center: IVec3, chunks: &[(IVec3, Chunk)]) -> Vec<(IVec3, Block)> {
        let bounds = WaterBounds::around(center);
        self.seed_loaded_water(bounds, chunks);
        self.remove_blocked_cells(bounds, chunks);

        let active: Vec<IVec3> = self
            .cells
            .keys()
            .copied()
            .filter(|pos| bounds.contains(*pos))
            .collect();
        let mut deltas = HashMap::new();
        let mut affected = HashSet::new();

        for pos in active {
            let mass = self.mass_with_delta(pos, &deltas);
            if mass <= WATER_MIN_MASS {
                continue;
            }

            let below = pos + IVec3::NEG_Y;
            if can_hold_water(below, chunks) {
                let below_mass = self.mass_with_delta(below, &deltas);
                let capacity = WATER_FULL_MASS - below_mass;
                let flow = (capacity.min(mass) * WATER_DOWN_FLOW).clamp(0.0, WATER_SIDE_FLOW * 3.0);
                self.move_mass(pos, below, flow, &mut deltas, &mut affected);
            }

            let mass = self.mass_with_delta(pos, &deltas);
            if mass <= WATER_MIN_MASS {
                continue;
            }

            for offset in SIDE_NEIGHBORS {
                let target = pos + offset;
                if !can_hold_water(target, chunks) {
                    continue;
                }

                let target_mass = self.mass_with_delta(target, &deltas);
                let flow = ((mass - target_mass) * 0.25).clamp(0.0, WATER_SIDE_FLOW);
                self.move_mass(pos, target, flow, &mut deltas, &mut affected);
            }
        }

        for (pos, delta) in deltas {
            let mass = (self.mass(pos) + delta).clamp(0.0, WATER_FULL_MASS);
            if mass <= WATER_MIN_MASS {
                self.cells.remove(&pos);
            } else {
                self.cells.insert(pos, WaterCell { mass });
            }
        }

        self.block_changes(affected, chunks)
    }

    fn seed_loaded_water(&mut self, bounds: WaterBounds, chunks: &[(IVec3, Chunk)]) {
        for y in bounds.min_y..=bounds.max_y {
            for z in bounds.min_z..=bounds.max_z {
                for x in bounds.min_x..=bounds.max_x {
                    let pos = IVec3::new(x, y, z);
                    if block_from_snapshot(pos, chunks) == Block::Water {
                        self.cells.entry(pos).or_insert(WaterCell {
                            mass: WATER_FULL_MASS,
                        });
                    }
                }
            }
        }
    }

    fn remove_blocked_cells(&mut self, bounds: WaterBounds, chunks: &[(IVec3, Chunk)]) {
        self.cells.retain(|pos, _| {
            !bounds.contains(*pos) || !block_from_snapshot(*pos, chunks).is_solid()
        });
    }

    fn move_mass(
        &self,
        from: IVec3,
        to: IVec3,
        amount: f32,
        deltas: &mut HashMap<IVec3, f32>,
        affected: &mut HashSet<IVec3>,
    ) {
        if amount <= WATER_MIN_MASS {
            return;
        }

        let available = self.mass_with_delta(from, deltas);
        let amount = amount.min((available - WATER_MIN_MASS).max(0.0));
        if amount <= WATER_MIN_MASS {
            return;
        }

        *deltas.entry(from).or_insert(0.0) -= amount;
        *deltas.entry(to).or_insert(0.0) += amount;
        affected.insert(from);
        affected.insert(to);
    }

    fn block_changes(
        &self,
        affected: HashSet<IVec3>,
        chunks: &[(IVec3, Chunk)],
    ) -> Vec<(IVec3, Block)> {
        affected
            .into_iter()
            .filter_map(|pos| {
                let block = block_from_snapshot(pos, chunks);
                if block.is_solid() {
                    return None;
                }

                let mass = self.mass(pos);
                if mass >= WATER_VISIBLE_MASS && block == Block::Air {
                    Some((pos, Block::Water))
                } else if mass < WATER_VISIBLE_MASS && block == Block::Water {
                    Some((pos, Block::Air))
                } else {
                    None
                }
            })
            .collect()
    }

    fn mass(&self, pos: IVec3) -> f32 {
        self.cells.get(&pos).map(|cell| cell.mass).unwrap_or(0.0)
    }

    fn mass_with_delta(&self, pos: IVec3, deltas: &HashMap<IVec3, f32>) -> f32 {
        (self.mass(pos) + deltas.get(&pos).copied().unwrap_or(0.0)).clamp(0.0, WATER_FULL_MASS)
    }
}

#[derive(Clone, Copy)]
struct WaterBounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
}

impl WaterBounds {
    fn around(center: IVec3) -> Self {
        Self {
            min_x: center.x - WATER_RADIUS,
            max_x: center.x + WATER_RADIUS,
            min_y: (center.y - WATER_Y_RANGE).max(1),
            max_y: (center.y + WATER_Y_RANGE).clamp(1, CHUNK_HEIGHT as i32 - 2),
            min_z: center.z - WATER_RADIUS,
            max_z: center.z + WATER_RADIUS,
        }
    }

    fn contains(self, pos: IVec3) -> bool {
        pos.x >= self.min_x
            && pos.x <= self.max_x
            && pos.y >= self.min_y
            && pos.y <= self.max_y
            && pos.z >= self.min_z
            && pos.z <= self.max_z
    }
}

fn can_hold_water(pos: IVec3, chunks: &[(IVec3, Chunk)]) -> bool {
    matches!(block_from_snapshot(pos, chunks), Block::Air | Block::Water)
}

fn block_from_snapshot(world_pos: IVec3, chunks: &[(IVec3, Chunk)]) -> Block {
    for (origin, chunk) in chunks {
        let local = world_pos - *origin;
        if local.x >= 0
            && local.y >= 0
            && local.z >= 0
            && local.x < CHUNK_SIZE as i32
            && local.y < CHUNK_HEIGHT as i32
            && local.z < CHUNK_SIZE as i32
        {
            return chunk.get(local.x, local.y, local.z);
        }
    }
    Block::Air
}

const SIDE_NEIGHBORS: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
