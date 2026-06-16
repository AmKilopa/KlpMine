use bevy::prelude::*;

const DROPPED_ITEM_HALF_EXTENT: f32 = 0.16;
const DROPPED_ITEM_COLLISION_STEP: f32 = 0.045;
const MAX_DROPPED_ITEM_SPEED: f32 = 7.0;
const STOP_SPEED_SQUARED: f32 = 0.035;

pub fn clamp_dropped_item_velocity(velocity: Vec3) -> Vec3 {
    let speed = velocity.length();
    if speed > MAX_DROPPED_ITEM_SPEED {
        velocity / speed * MAX_DROPPED_ITEM_SPEED
    } else {
        velocity
    }
}

pub fn move_dropped_item(
    position: &mut Vec3,
    velocity: &mut Vec3,
    dt: f32,
    mut is_solid: impl FnMut(IVec3) -> bool,
) {
    *velocity = clamp_dropped_item_velocity(*velocity);
    let delta = *velocity * dt;

    if move_axis(position, Vec3::X * delta.x, &mut is_solid) {
        velocity.x *= -0.16;
    }

    if move_axis(position, Vec3::Y * delta.y, &mut is_solid) {
        let falling = velocity.y < 0.0;
        velocity.y *= if falling { -0.18 } else { -0.08 };
        velocity.x *= 0.58;
        velocity.z *= 0.58;
    }

    if move_axis(position, Vec3::Z * delta.z, &mut is_solid) {
        velocity.z *= -0.16;
    }

    if velocity.length_squared() < STOP_SPEED_SQUARED {
        *velocity = Vec3::ZERO;
    }
}

fn move_axis(position: &mut Vec3, delta: Vec3, is_solid: &mut impl FnMut(IVec3) -> bool) -> bool {
    if delta.length_squared() == 0.0 {
        return false;
    }

    let steps = (delta.length() / DROPPED_ITEM_COLLISION_STEP)
        .ceil()
        .max(1.0) as usize;
    let step = delta / steps as f32;

    for _ in 0..steps {
        let next = *position + step;
        if dropped_item_collides(next, is_solid) {
            return true;
        }
        *position = next;
    }

    false
}

fn dropped_item_collides(position: Vec3, is_solid: &mut impl FnMut(IVec3) -> bool) -> bool {
    let min = position - Vec3::splat(DROPPED_ITEM_HALF_EXTENT);
    let max = position + Vec3::splat(DROPPED_ITEM_HALF_EXTENT);

    for y in min.y.floor() as i32..=max.y.floor() as i32 {
        for z in min.z.floor() as i32..=max.z.floor() as i32 {
            for x in min.x.floor() as i32..=max.x.floor() as i32 {
                if is_solid(IVec3::new(x, y, z)) {
                    return true;
                }
            }
        }
    }

    false
}
