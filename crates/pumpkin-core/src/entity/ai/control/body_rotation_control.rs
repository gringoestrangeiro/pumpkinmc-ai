use crate::entity::ai::control::Control;
use crate::entity::mob::Mob;
use pumpkin_util::math::rotate_if_necessary;

const HEAD_STABLE_ANGLE: f32 = 15.0;
const DELAY_UNTIL_STARTING_TO_FACE_FORWARD: i32 = 10;
const HOW_LONG_IT_TAKES_TO_FACE_FORWARD: i32 = 10;
/// Vanilla `MoveControl.MIN_SPEED_SQR`.
const MIN_SPEED_SQR: f32 = 2.500_000_3E-7;

pub struct BodyRotationControl {
    head_stable_time: i32,
    last_stable_y_head_rot: f32,
}

impl Default for BodyRotationControl {
    fn default() -> Self {
        Self::new()
    }
}

impl BodyRotationControl {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            head_stable_time: 0,
            last_stable_y_head_rot: 0.0,
        }
    }

    pub fn client_tick(&mut self, mob: &dyn Mob) {
        let mob_entity = mob.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;

        if Self::is_moving(mob) {
            entity.body_yaw.store(entity.yaw.load());
            Self::rotate_head_if_necessary(mob);
            self.last_stable_y_head_rot = entity.head_yaw.load();
            self.head_stable_time = 0;
        } else if Self::not_carrying_mob_passengers(mob) {
            if (entity.head_yaw.load() - self.last_stable_y_head_rot).abs() > HEAD_STABLE_ANGLE {
                self.head_stable_time = 0;
                self.last_stable_y_head_rot = entity.head_yaw.load();
                Self::rotate_body_if_necessary(mob);
            } else {
                self.head_stable_time += 1;
                if self.head_stable_time > DELAY_UNTIL_STARTING_TO_FACE_FORWARD {
                    self.rotate_head_towards_front(mob);
                }
            }
        }
    }

    fn rotate_body_if_necessary(mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        let max_head_y_rot = mob.get_max_head_rotation();
        entity.body_yaw.store(rotate_if_necessary(
            entity.body_yaw.load(),
            entity.head_yaw.load(),
            max_head_y_rot,
        ));
    }

    fn rotate_head_if_necessary(mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        let max_head_y_rot = mob.get_max_head_rotation();
        entity.head_yaw.store(rotate_if_necessary(
            entity.head_yaw.load(),
            entity.body_yaw.load(),
            max_head_y_rot,
        ));
    }

    fn rotate_head_towards_front(&self, mob: &dyn Mob) {
        let time_since_starting = self.head_stable_time - DELAY_UNTIL_STARTING_TO_FACE_FORWARD;
        let face_forward_fraction =
            (time_since_starting as f32 / HOW_LONG_IT_TAKES_TO_FACE_FORWARD as f32).clamp(0.0, 1.0);
        let angle_remaining = mob.get_max_head_rotation() * (1.0 - face_forward_fraction);
        let entity = &mob.get_mob_entity().living_entity.entity;
        entity.body_yaw.store(rotate_if_necessary(
            entity.body_yaw.load(),
            entity.head_yaw.load(),
            angle_remaining,
        ));
    }

    fn not_carrying_mob_passengers(mob: &dyn Mob) -> bool {
        mob.get_entity()
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .is_none_or(|passenger| passenger.get_mob().is_none())
    }

    /// Vanilla compares against `xo`/`zo`, the position at the start of this tick.
    fn is_moving(mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let moved = entity.pos.load() - entity.last_pos.load();
        moved.x * moved.x + moved.z * moved.z > f64::from(MIN_SPEED_SQR)
    }
}

impl Control for BodyRotationControl {}
