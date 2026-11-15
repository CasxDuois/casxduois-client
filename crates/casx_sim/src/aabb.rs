// crates/casx_sim/src/aabb.rs
// Giai toan va cham Swept AABB voi the gioi Voxel va buoc nhay bac thang step-up 0.6m

use casx_core::jmath::{AABB, DVec3, IVec3};
use casx_world::WorldStorage;

/// Tinh toan do doi chuyen thuc te cua thuc the sau khi giai quyet va cham voi cac khoi ran
pub fn move_entity_with_collisions(
    world: &WorldStorage,
    bbox: AABB,
    velocity: DVec3,
) -> (DVec3, bool) {
    let mut allowed = velocity;

    // 1. Thu thap tat ca cac hop bao khoi ran xung quanh duong di
    let min_x = (bbox.min.x + velocity.x.min(0.0)).floor() as i32 - 1;
    let max_x = (bbox.max.x + velocity.x.max(0.0)).ceil() as i32 + 1;
    let min_y = (bbox.min.y + velocity.y.min(0.0)).floor() as i32 - 1;
    let max_y = (bbox.max.y + velocity.y.max(0.0)).ceil() as i32 + 1;
    let min_z = (bbox.min.z + velocity.z.min(0.0)).floor() as i32 - 1;
    let max_z = (bbox.max.z + velocity.z.max(0.0)).ceil() as i32 + 1;

    let mut potential_colliders = Vec::new();
    for bx in min_x..=max_x {
        for by in min_y..=max_y {
            for bz in min_z..=max_z {
                if world.is_solid_block(bx, by, bz) {
                    potential_colliders.push(AABB::from_block(IVec3::new(bx, by, bz)));
                }
            }
        }
    }

    // 2. Va cham theo truc Y (Uu tien so 1)
    for block_box in &potential_colliders {
        allowed.y = block_box.calculate_y_offset(&bbox, allowed.y);
    }
    let after_y = bbox.offset(DVec3::new(0.0, allowed.y, 0.0));

    // 3. Va cham theo truc X
    for block_box in &potential_colliders {
        allowed.x = block_box.calculate_x_offset(&after_y, allowed.x);
    }
    let after_x = after_y.offset(DVec3::new(allowed.x, 0.0, 0.0));

    // 4. Va cham theo truc Z
    for block_box in &potential_colliders {
        allowed.z = block_box.calculate_z_offset(&after_x, allowed.z);
    }

    // Kiem tra tiep dat: Neu van toc ban dau huong xuong va bi can tro boi vat the ben duoi
    let on_ground = velocity.y < 0.0 && allowed.y > velocity.y;

    (allowed, on_ground)
}

/// Trinh mo phong vat ly nguoi choi 20 TPS
#[derive(Debug, Clone)]
pub struct PlayerSimulation {
    pub pos: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl PlayerSimulation {
    pub fn new(pos: DVec3) -> Self {
        Self {
            pos,
            velocity: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
        }
    }

    pub fn set_position(&mut self, pos: DVec3, yaw: f32, pitch: f32) {
        self.pos = pos;
        self.yaw = yaw;
        self.pitch = pitch;
        self.velocity = DVec3::ZERO;
        self.on_ground = false;
    }

    /// Tinh toan 1 tick vat ly (50ms - 20 TPS)
    pub fn tick(&mut self, world: &WorldStorage) {
        // Ap dung trong luc chuan Minecraft: v_y = (v_y - 0.08) * 0.98
        self.velocity.y = (self.velocity.y - 0.08) * 0.98;
        // Ma sat khong khi ngang: v_x, v_z * 0.91
        self.velocity.x *= 0.91;
        self.velocity.z *= 0.91;

        let player_box = AABB::player_box(self.pos);
        let (allowed_move, on_ground) = move_entity_with_collisions(world, player_box, self.velocity);

        self.pos += allowed_move;
        self.on_ground = on_ground;

        if self.on_ground {
            self.velocity.y = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use casx_world::region::{ChunkColumnV340, ChunkSectionV340};
    use casx_core::palette::SectionPaletteV340;

    #[test]
    fn test_simulation_freefall_and_landing() {
        let mut world = WorldStorage::new();
        // Dat mat dat tai Y = 63 (san da)
        let mut data = vec![0u64; 256];
        data[0] |= 1u64; // Block tai (0, 0, 0) cua section 4 (Y = 64..79)
        let palette = vec![0, 16]; // stone
        let section = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        let mut col = ChunkColumnV340::new(0, 0);
        col.sections.insert(4, ChunkSectionV340 {
            y_index: 4,
            palette: section,
            block_light: vec![0; 2048],
            sky_light: None,
        });
        world.chunks.insert((0, 0), col);

        // Nguoi choi xuat phat tai (0.5, 66.0, 0.5)
        let mut sim = PlayerSimulation::new(DVec3::new(0.5, 66.0, 0.5));
        assert!(!sim.on_ground);

        // Chay 20 ticks
        for _ in 0..20 {
            sim.tick(&world);
        }

        // Sau 20 ticks, nguoi choi phai tiep dat vung chac tai mat san khoi da Y = 65.0 (vi block tai 64 day den 65)
        assert!(sim.on_ground);
        assert!((sim.pos.y - 65.0).abs() < 1e-4);
        assert_eq!(sim.velocity.y, 0.0);
    }
}
