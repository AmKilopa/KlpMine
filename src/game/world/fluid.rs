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
    sources: HashSet<IVec3>,
    last_changes: usize,
}

#[derive(Clone, Copy, Default)]
pub struct WaterDebugStats {
    pub active_cells: usize,
    pub source_cells: usize,
    pub visible_cells: usize,
    pub total_mass: f32,
    pub last_changes: usize,
}

const WATER_TICK_SECONDS: f32 = 0.12;
const WATER_RADIUS: i32 = 10;
const WATER_Y_RANGE: i32 = 8;
const WATER_FULL_MASS: f32 = 1.0;
const WATER_MIN_MASS: f32 = 0.015;
const WATER_VISIBLE_MASS: f32 = 0.08;
const WATER_DOWN_FLOW: f32 = 0.72;
const WATER_SIDE_FLOW: f32 = 0.24;
const WATER_DIAGONAL_FLOW: f32 = 0.16;

impl WaterSimulation {
    pub fn new() -> Self {
        Self {
            timer: Timer::from_seconds(WATER_TICK_SECONDS, TimerMode::Repeating),
            cells: HashMap::new(),
            sources: HashSet::new(),
            last_changes: 0,
        }
    }

    pub fn tick(&mut self, delta: Duration) -> bool {
        self.timer.tick(delta);
        self.timer.just_finished()
    }

    pub fn clear(&mut self, pos: IVec3) {
        self.cells.remove(&pos);
        self.sources.remove(&pos);
    }

    pub fn debug_stats(&self) -> WaterDebugStats {
        WaterDebugStats {
            active_cells: self.cells.len(),
            source_cells: self.sources.len(),
            visible_cells: self
                .cells
                .values()
                .filter(|cell| cell.mass >= WATER_VISIBLE_MASS)
                .count(),
            total_mass: self.cells.values().map(|cell| cell.mass).sum(),
            last_changes: self.last_changes,
        }
    }

    pub fn fill_fraction_for_block(&self, pos: IVec3, block: Block) -> f32 {
        if let Some(cell) = self.cells.get(&pos) {
            cell.mass.clamp(0.0, WATER_FULL_MASS)
        } else if block == Block::Water {
            WATER_FULL_MASS
        } else {
            0.0
        }
    }

    pub fn simulate(&mut self, center: IVec3, chunks: &[(IVec3, Chunk)]) -> Vec<(IVec3, Block)> {
        let bounds = WaterBounds::around(center);
        self.seed_loaded_water(bounds, chunks);
        self.remove_blocked_cells(bounds, chunks);

        let mut active: Vec<IVec3> = self
            .cells
            .keys()
            .copied()
            .filter(|pos| bounds.contains(*pos))
            .collect();
        active.sort_by_key(|pos| (pos.y, pos.x, pos.z));
        active.reverse();

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
            if can_hold_water(below, chunks) {
                continue;
            }

            for offset in ORTHOGONAL_NEIGHBORS {
                let target = pos + offset;
                if !can_hold_water(target, chunks) {
                    continue;
                }

                let target_mass = self.mass_with_delta(target, &deltas);
                let flow = ((mass - target_mass) * 0.25).clamp(0.0, WATER_SIDE_FLOW);
                self.move_mass(pos, target, flow, &mut deltas, &mut affected);
            }

            let mass = self.mass_with_delta(pos, &deltas);
            if mass <= WATER_MIN_MASS {
                continue;
            }

            for offset in DIAGONAL_NEIGHBORS {
                let target = pos + offset;
                if !diagonal_path_open(pos, offset, chunks) {
                    continue;
                }

                let target_mass = self.mass_with_delta(target, &deltas);
                let flow = ((mass - target_mass) * 0.18).clamp(0.0, WATER_DIAGONAL_FLOW);
                self.move_mass(pos, target, flow, &mut deltas, &mut affected);
            }
        }

        for (pos, delta) in deltas {
            let base = if self.sources.contains(&pos) {
                WATER_FULL_MASS
            } else {
                self.mass(pos)
            };
            let mass = (base + delta).clamp(0.0, WATER_FULL_MASS);
            if mass <= WATER_MIN_MASS {
                self.cells.remove(&pos);
            } else {
                self.cells.insert(pos, WaterCell { mass });
            }
        }

        for source in self.sources.iter().copied().collect::<Vec<_>>() {
            if bounds.contains(source) && can_hold_water(source, chunks) {
                self.cells.insert(
                    source,
                    WaterCell {
                        mass: WATER_FULL_MASS,
                    },
                );
                affected.insert(source);
            }
        }

        let changes = self.block_changes(affected, chunks);
        self.last_changes = changes.len();
        changes
    }

    fn seed_loaded_water(&mut self, bounds: WaterBounds, chunks: &[(IVec3, Chunk)]) {
        for y in bounds.min_y..=bounds.max_y {
            for z in bounds.min_z..=bounds.max_z {
                for x in bounds.min_x..=bounds.max_x {
                    let pos = IVec3::new(x, y, z);
                    if block_from_snapshot(pos, chunks) == Some(Block::Water) {
                        let existed = self.cells.contains_key(&pos);
                        self.cells.entry(pos).or_insert(WaterCell {
                            mass: WATER_FULL_MASS,
                        });
                        if !existed {
                            self.sources.insert(pos);
                        }
                    }
                }
            }
        }
    }

    fn remove_blocked_cells(&mut self, bounds: WaterBounds, chunks: &[(IVec3, Chunk)]) {
        self.cells.retain(|pos, _| {
            !bounds.contains(*pos)
                || block_from_snapshot(*pos, chunks).is_some_and(|block| !block.is_solid())
        });
        self.sources.retain(|pos| {
            self.cells.contains_key(pos)
                && (!bounds.contains(*pos)
                    || block_from_snapshot(*pos, chunks).is_some_and(|block| !block.is_solid()))
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

        if !self.sources.contains(&from) {
            *deltas.entry(from).or_insert(0.0) -= amount;
        }
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
                let block = block_from_snapshot(pos, chunks)?;
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
        let base = if self.sources.contains(&pos) {
            WATER_FULL_MASS
        } else {
            self.mass(pos)
        };
        (base + deltas.get(&pos).copied().unwrap_or(0.0)).clamp(0.0, WATER_FULL_MASS)
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
    matches!(
        block_from_snapshot(pos, chunks),
        Some(Block::Air | Block::Water)
    )
}

fn block_from_snapshot(world_pos: IVec3, chunks: &[(IVec3, Chunk)]) -> Option<Block> {
    for (origin, chunk) in chunks {
        let local = world_pos - *origin;
        if local.x >= 0
            && local.y >= 0
            && local.z >= 0
            && local.x < CHUNK_SIZE as i32
            && local.y < CHUNK_HEIGHT as i32
            && local.z < CHUNK_SIZE as i32
        {
            return Some(chunk.get(local.x, local.y, local.z));
        }
    }
    None
}

fn diagonal_path_open(pos: IVec3, offset: IVec3, chunks: &[(IVec3, Chunk)]) -> bool {
    if !can_hold_water(pos + offset, chunks) {
        return false;
    }

    let x_step = IVec3::new(offset.x, 0, 0);
    let z_step = IVec3::new(0, 0, offset.z);

    can_hold_water(pos + x_step, chunks) || can_hold_water(pos + z_step, chunks)
}

const ORTHOGONAL_NEIGHBORS: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
const DIAGONAL_NEIGHBORS: [IVec3; 4] = [
    IVec3::new(1, 0, 1),
    IVec3::new(1, 0, -1),
    IVec3::new(-1, 0, 1),
    IVec3::new(-1, 0, -1),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_snapshot() -> Vec<(IVec3, Chunk)> {
        vec![(IVec3::ZERO, Chunk::empty())]
    }

    #[test]
    fn dynamic_water_conserves_mass() {
        let mut simulation = WaterSimulation::new();
        simulation
            .cells
            .insert(IVec3::new(3, 8, 3), WaterCell { mass: 1.0 });

        simulation.simulate(IVec3::new(3, 8, 3), &empty_snapshot());

        let total_mass: f32 = simulation.cells.values().map(|cell| cell.mass).sum();
        assert!((total_mass - 1.0).abs() < 0.001);
    }

    #[test]
    fn loaded_water_becomes_source() {
        let mut chunk = Chunk::empty();
        chunk.set(3, 8, 3, Block::Water);
        let mut simulation = WaterSimulation::new();

        simulation.simulate(IVec3::new(3, 8, 3), &[(IVec3::ZERO, chunk)]);

        let pos = IVec3::new(3, 8, 3);
        assert!(simulation.sources.contains(&pos));
        assert_eq!(simulation.mass(pos), WATER_FULL_MASS);
    }
}
