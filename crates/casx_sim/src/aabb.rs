// crates/casx_sim/src/aabb.rs
// Swept AABB collision resolution against voxel terrain.

use casx_core::jmath::{DVec3, IVec3, AABB};
use casx_world::WorldStorage;

/// Calculates entity displacement after resolving collisions against solid blocks (Y -> X -> Z).
pub fn move_entity_with_collisions(
    world: &WorldStorage,
    bbox: AABB,
    velocity: DVec3,
) -> (DVec3, bool) {
    let mut allowed = velocity;

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

    // Resolve Y axis collision first
    for block_box in &potential_colliders {
        allowed.y = block_box.calculate_y_offset(&bbox, allowed.y);
    }
    let after_y = bbox.offset(DVec3::new(0.0, allowed.y, 0.0));

    // Resolve X axis collision second
    for block_box in &potential_colliders {
        allowed.x = block_box.calculate_x_offset(&after_y, allowed.x);
    }
    let after_x = after_y.offset(DVec3::new(allowed.x, 0.0, 0.0));

    // Resolve Z axis collision third
    for block_box in &potential_colliders {
        allowed.z = block_box.calculate_z_offset(&after_x, allowed.z);
    }

    let on_ground = velocity.y < 0.0 && allowed.y > velocity.y;

    (allowed, on_ground)
}

/// 20 TPS physics simulation runner for local player.
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

    /// Simulates 1 physics tick (50ms - 20 TPS)
    pub fn tick(&mut self, world: &WorldStorage) {
        // Apply Vanilla gravity: v_y = (v_y - 0.08) * 0.98
        self.velocity.y = (self.velocity.y - 0.08) * 0.98;
        // Horizontal drag: v_x, v_z * 0.91
        self.velocity.x *= 0.91;
        self.velocity.z *= 0.91;

        let player_box = AABB::player_box(self.pos);
        let (allowed_move, on_ground) =
            move_entity_with_collisions(world, player_box, self.velocity);

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
    use casx_core::palette::SectionPaletteV340;
    use casx_world::region::{ChunkColumnV340, ChunkSectionV340};

    #[test]
    fn test_simulation_freefall_and_landing() {
        let mut world = WorldStorage::new();
        // Place stone ground at Y = 64
        let mut data = vec![0u64; 256];
        data[0] |= 1u64; // Block at (0, 0, 0) of section 4 (Y = 64..79)
        let palette = vec![0, 16]; // stone
        let section = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        let mut col = ChunkColumnV340::new(0, 0);
        col.sections.insert(
            4,
            ChunkSectionV340::new_v340(4, section, vec![0; 2048], None),
        );
        world.chunks.insert((0, 0), col);

        // Player spawns at (0.5, 66.0, 0.5)
        let mut sim = PlayerSimulation::new(DVec3::new(0.5, 66.0, 0.5));
        assert!(!sim.on_ground);

        // Run 20 ticks
        for _ in 0..20 {
            sim.tick(&world);
        }

        // After 20 ticks, player lands securely on stone surface at Y = 65.0
        assert!(sim.on_ground);
        assert!((sim.pos.y - 65.0).abs() < 1e-4);
        assert_eq!(sim.velocity.y, 0.0);
    }
}
