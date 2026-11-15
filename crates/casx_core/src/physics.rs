//! He thong vat ly nguoi choi 20 TPS (AABB Swept Collision, trong truong, ma sat, buoc nhay bac thang).

use crate::world::ClientWorldCache;
use glam::{DVec3, IVec3};

/// Hop bao truc song song (Axis-Aligned Bounding Box)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    pub fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    pub fn from_block(pos: IVec3) -> Self {
        Self {
            min: DVec3::new(pos.x as f64, pos.y as f64, pos.z as f64),
            max: DVec3::new((pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64),
        }
    }

    pub fn offset(&self, d: DVec3) -> Self {
        Self {
            min: self.min + d,
            max: self.max + d,
        }
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.max.x > other.min.x
            && self.min.x < other.max.x
            && self.max.y > other.min.y
            && self.min.y < other.max.y
            && self.max.z > other.min.z
            && self.min.z < other.max.z
    }

    /// Tinh toan do lech theo truc Y de tranh va cham voi hop bao khac
    pub fn calculate_y_offset(&self, other: &Aabb, mut offset_y: f64) -> f64 {
        if other.max.x > self.min.x && other.min.x < self.max.x && other.max.z > self.min.z && other.min.z < self.max.z {
            if offset_y > 0.0 && other.max.y <= self.min.y {
                let d = self.min.y - other.max.y;
                if d < offset_y {
                    offset_y = d;
                }
            } else if offset_y < 0.0 && other.min.y >= self.max.y {
                let d = self.max.y - other.min.y;
                if d > offset_y {
                    offset_y = d;
                }
            }
        }
        offset_y
    }

    /// Tinh toan do lech theo truc X de tranh va cham
    pub fn calculate_x_offset(&self, other: &Aabb, mut offset_x: f64) -> f64 {
        if other.max.y > self.min.y && other.min.y < self.max.y && other.max.z > self.min.z && other.min.z < self.max.z {
            if offset_x > 0.0 && other.max.x <= self.min.x {
                let d = self.min.x - other.max.x;
                if d < offset_x {
                    offset_x = d;
                }
            } else if offset_x < 0.0 && other.min.x >= self.max.x {
                let d = self.max.x - other.min.x;
                if d > offset_x {
                    offset_x = d;
                }
            }
        }
        offset_x
    }

    /// Tinh toan do lech theo truc Z de tranh va cham
    pub fn calculate_z_offset(&self, other: &Aabb, mut offset_z: f64) -> f64 {
        if other.max.x > self.min.x && other.min.x < self.max.x && other.max.y > self.min.y && other.min.y < self.max.y {
            if offset_z > 0.0 && other.max.z <= self.min.z {
                let d = self.min.z - other.max.z;
                if d < offset_z {
                    offset_z = d;
                }
            } else if offset_z < 0.0 && other.min.z >= self.max.z {
                let d = self.max.z - other.min.z;
                if d > offset_z {
                    offset_z = d;
                }
            }
        }
        offset_z
    }
}

/// Bo dieu khien vat ly nguoi choi tai cho (20 TPS)
#[derive(Debug, Clone)]
pub struct LocalPlayerPhysics {
    pub curr_pos: DVec3,
    pub prev_pos: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub is_sneaking: bool,
    pub is_sprinting: bool,
    pub step_height: f64,
}

impl Default for LocalPlayerPhysics {
    fn default() -> Self {
        Self {
            curr_pos: DVec3::ZERO,
            prev_pos: DVec3::ZERO,
            velocity: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            is_sneaking: false,
            is_sprinting: false,
            step_height: 0.6, // Chieu cao buoc len bac thang
        }
    }
}

impl LocalPlayerPhysics {
    pub fn bounding_box(&self) -> Aabb {
        let half_w = 0.3; // Chieu rong nguoi choi 0.6m (ban kinh 0.3m)
        let height = if self.is_sneaking { 1.5 } else { 1.8 };
        Aabb::new(
            DVec3::new(self.curr_pos.x - half_w, self.curr_pos.y, self.curr_pos.z - half_w),
            DVec3::new(self.curr_pos.x + half_w, self.curr_pos.y + height, self.curr_pos.z + half_w),
        )
    }

    /// Lay danh sach hop bao cua cac khoi dac xung quanh nguoi choi
    fn get_surrounding_boxes(&self, world: &ClientWorldCache, box_area: &Aabb) -> Vec<Aabb> {
        let min_x = box_area.min.x.floor() as i32;
        let max_x = box_area.max.x.ceil() as i32;
        let min_y = box_area.min.y.floor() as i32;
        let max_y = box_area.max.y.ceil() as i32;
        let min_z = box_area.min.z.floor() as i32;
        let max_z = box_area.max.z.ceil() as i32;

        let mut boxes = Vec::new();
        for y in min_y..max_y {
            for z in min_z..max_z {
                for x in min_x..max_x {
                    let block = world.get_block(x, y, z);
                    if block.is_solid() {
                        boxes.push(Aabb::from_block(IVec3::new(x, y, z)));
                    }
                }
            }
        }
        boxes
    }

    /// Thuc thi mot tick vat ly $20\text{ TPS}$ ($\Delta t = 50\text{ ms}$)
    pub fn step_physics_tick(
        &mut self,
        world: &ClientWorldCache,
        forward_input: f32,
        strafe_input: f32,
        jump_input: bool,
    ) {
        self.prev_pos = self.curr_pos;

        // 1. Xu ly Nhay (Jump)
        if self.on_ground && jump_input {
            self.velocity.y = 0.42;
            if self.is_sprinting {
                let rad = self.yaw.to_radians();
                self.velocity.x -= (rad.sin() as f64) * 0.2;
                self.velocity.z += (rad.cos() as f64) * 0.2;
            }
        }

        // 2. Ap dung trong luc theo cong thuc Vanilla truoc khi di chuyen
        // $$v_y^{(t+1)} = (v_y^{(t)} - 0.08) \times 0.98$$
        self.velocity.y = (self.velocity.y - 0.08) * 0.98;

        // 3. Tinh toan gia toc di chuyen theo huong nhin (Yaw)
        let mut move_x = 0.0f64;
        let mut move_z = 0.0f64;
        let input_len_sq = forward_input * forward_input + strafe_input * strafe_input;
        if input_len_sq > 1e-4 {
            let inv_len = 1.0 / (input_len_sq as f64).sqrt();
            let norm_forward = (forward_input as f64) * inv_len;
            let norm_strafe = (strafe_input as f64) * inv_len;

            let speed = if self.on_ground {
                if self.is_sprinting { 0.13 } else { 0.10 }
            } else {
                0.02
            };

            let rad = (self.yaw as f64).to_radians();
            let sin_yaw = rad.sin();
            let cos_yaw = rad.cos();

            move_x = (norm_strafe * cos_yaw - norm_forward * sin_yaw) * speed;
            move_z = (norm_forward * cos_yaw + norm_strafe * sin_yaw) * speed;
        }

        self.velocity.x += move_x;
        self.velocity.z += move_z;

        // 4. Swept AABB Collision voi the gioi
        let mut player_box = self.bounding_box();
        let motion = self.velocity;

        // Vung quet AABB bao gom ca vi tri hien tai va vi tri du kien
        let expanded_box = Aabb::new(
            DVec3::new(
                player_box.min.x.min(player_box.min.x + motion.x),
                player_box.min.y.min(player_box.min.y + motion.y),
                player_box.min.z.min(player_box.min.z + motion.z),
            ),
            DVec3::new(
                player_box.max.x.max(player_box.max.x + motion.x),
                player_box.max.y.max(player_box.max.y + motion.y),
                player_box.max.z.max(player_box.max.z + motion.z),
            ),
        );
        let surrounding = self.get_surrounding_boxes(world, &expanded_box);

        let mut actual_x = motion.x;
        let mut actual_y = motion.y;
        let mut actual_z = motion.z;

        // Va cham truc Y truoc (de xac dinh on_ground)
        for b in &surrounding {
            actual_y = b.calculate_y_offset(&player_box, actual_y);
        }
        player_box = player_box.offset(DVec3::new(0.0, actual_y, 0.0));

        // Va cham truc X
        for b in &surrounding {
            actual_x = b.calculate_x_offset(&player_box, actual_x);
        }
        player_box = player_box.offset(DVec3::new(actual_x, 0.0, 0.0));

        // Va cham truc Z
        for b in &surrounding {
            actual_z = b.calculate_z_offset(&player_box, actual_z);
        }

        // Cap nhat trang thai on_ground
        self.on_ground = motion.y < 0.0 && actual_y != motion.y;

        if motion.x != actual_x {
            self.velocity.x = 0.0;
        }
        if motion.z != actual_z {
            self.velocity.z = 0.0;
        }
        if motion.y != actual_y {
            self.velocity.y = 0.0;
        }

        self.curr_pos.x += actual_x;
        self.curr_pos.y += actual_y;
        self.curr_pos.z += actual_z;

        // 5. Ma sat mat dat / khong khi
        let friction = if self.on_ground { 0.6 } else { 1.0 };
        let drag = 0.91 * friction;
        self.velocity.x *= drag;
        self.velocity.z *= drag;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::CanonicalBlockState;

    #[test]
    fn test_freefall_gravity_parity_vanilla() {
        let world = ClientWorldCache::new();
        let mut player = LocalPlayerPhysics::default();
        player.curr_pos = DVec3::new(0.0, 100.0, 0.0);

        // 10 ticks roi tu do trong khong trung
        for _ in 0..10 {
            player.step_physics_tick(&world, 0.0, 0.0, false);
        }

        // Theo phuong trinh v_y = (v_y - 0.08) * 0.98, sau 10 ticks:
        // Vi tri Y phai roi xuong khoang [95.5, 96.5]
        assert!(player.curr_pos.y > 95.5 && player.curr_pos.y < 96.5, "Y pos: {}", player.curr_pos.y);
        assert!(!player.on_ground);
    }

    #[test]
    fn test_ground_landing_sets_on_ground() {
        let mut world = ClientWorldCache::new();
        let mut chunk = crate::world::ChunkColumn::new(0, 0, 0, 16);
        let stone = CanonicalBlockState::new(1, 0);
        // Dat mot san da tai Y = 60
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_block(x, 60, z, stone);
            }
        }
        world.insert_chunk(chunk);

        let mut player = LocalPlayerPhysics::default();
        player.curr_pos = DVec3::new(8.0, 62.0, 8.0);

        // Cho roi 20 ticks
        for _ in 0..20 {
            player.step_physics_tick(&world, 0.0, 0.0, false);
        }

        // Nguoi choi phai dung vung tren mat khoi tai Y = 61.0
        assert_eq!(player.curr_pos.y, 61.0);
        assert!(player.on_ground);
        assert_eq!(player.velocity.y, 0.0);
    }
}
