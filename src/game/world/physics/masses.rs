pub const TREE_RADIUS: f32 = 0.44;
pub const TREE_GRIP_RAY_RADIUS: f32 = TREE_RADIUS + 0.2;

pub const PLAYER_MAX_LIFT_MASS: f32 = 14.0;
pub const TREE_DRAG_GRAB_DISTANCE: f32 = 5.2;
pub const TREE_DRAG_BREAK_DISTANCE: f32 = 5.8;
pub const TREE_DRAG_STRETCH_LIMIT: f32 = 3.6;

const TREE_BASE_MASS: f32 = 2.0;
const TREE_LOG_MASS: f32 = 1.5;
const TREE_MASS_CAP: f32 = 20.0;

pub fn falling_tree_mass(log_count: i32) -> f32 {
    let logs = log_count.max(1) as f32;
    (TREE_BASE_MASS + logs * TREE_LOG_MASS).min(TREE_MASS_CAP)
}
