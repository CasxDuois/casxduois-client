//! Thuat toan do tia Voxel 3D DDA (Amanatides & Woo) va Ray-AABB Hitbox cho CasxDuois Client.

use crate::physics::Aabb;
use crate::world::ClientWorldCache;
use glam::{DVec3, IVec3};

/// Cac mat cua khoi lap phuong theo quy uoc Minecraft
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFace {
    Bottom = 0, // Y-
    Top = 1,    // Y+
    North = 2,  // Z-
    South = 3,  // Z+
    West = 4,   // X-
    East = 5,   // X+
}

/// Ket qua va cham tia do voi mot khoi trong the gioi
#[derive(Debug, Clone, PartialEq)]
pub struct BlockHitResult {
    pub block_pos: IVec3,
    pub face: BlockFace,
    pub hit_point: DVec3,
    pub distance: f64,
}

/// Thuat toan Amanatides & Woo Fast Voxel Traversal (3D DDA)
///
/// Tim khoi dac dau tien ma tam nhin cua nguoi choi cat qua trong pham vi `max_reach` ($4.5\text{ m}$).
pub fn raycast_block(
    world: &ClientWorldCache,
    origin: DVec3,
    direction: DVec3,
    max_reach: f64,
) -> Option<BlockHitResult> {
    if direction.length_squared() < 1e-6 {
        return None;
    }
    let dir = direction.normalize();

    let mut voxel = IVec3::new(
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );

    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };

    let t_delta_x = if dir.x != 0.0 { (1.0 / dir.x).abs() } else { f64::INFINITY };
    let t_delta_y = if dir.y != 0.0 { (1.0 / dir.y).abs() } else { f64::INFINITY };
    let t_delta_z = if dir.z != 0.0 { (1.0 / dir.z).abs() } else { f64::INFINITY };

    let mut t_max_x = if dir.x > 0.0 {
        ((voxel.x + 1) as f64 - origin.x) * t_delta_x
    } else if dir.x < 0.0 {
        (origin.x - voxel.x as f64) * t_delta_x
    } else {
        f64::INFINITY
    };

    let mut t_max_y = if dir.y > 0.0 {
        ((voxel.y + 1) as f64 - origin.y) * t_delta_y
    } else if dir.y < 0.0 {
        (origin.y - voxel.y as f64) * t_delta_y
    } else {
        f64::INFINITY
    };

    let mut t_max_z = if dir.z > 0.0 {
        ((voxel.z + 1) as f64 - origin.z) * t_delta_z
    } else if dir.z < 0.0 {
        (origin.z - voxel.z as f64) * t_delta_z
    } else {
        f64::INFINITY
    };

    let mut last_face = BlockFace::Top;
    let mut traveled = 0.0f64;

    while traveled <= max_reach {
        let block = world.get_block(voxel.x, voxel.y, voxel.z);
        if block.is_solid() {
            return Some(BlockHitResult {
                block_pos: voxel,
                face: last_face,
                hit_point: origin + dir * traveled,
                distance: traveled,
            });
        }

        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                traveled = t_max_x;
                voxel.x += step_x;
                t_max_x += t_delta_x;
                last_face = if step_x > 0 { BlockFace::West } else { BlockFace::East };
            } else {
                traveled = t_max_z;
                voxel.z += step_z;
                t_max_z += t_delta_z;
                last_face = if step_z > 0 { BlockFace::North } else { BlockFace::South };
            }
        } else if t_max_y < t_max_z {
            traveled = t_max_y;
            voxel.y += step_y;
            t_max_y += t_delta_y;
            last_face = if step_y > 0 { BlockFace::Bottom } else { BlockFace::Top };
        } else {
            traveled = t_max_z;
            voxel.z += step_z;
            t_max_z += t_delta_z;
            last_face = if step_z > 0 { BlockFace::North } else { BlockFace::South };
        }
    }

    None
}

/// Kiem tra tia danh PvP cat qua hop bao AABB cua thuc the (Slab Intersection Method)
pub fn ray_intersects_aabb(origin: DVec3, dir: DVec3, box_aabb: &Aabb, max_reach: f64) -> Option<f64> {
    let inv_x = if dir.x != 0.0 { 1.0 / dir.x } else { f64::INFINITY };
    let inv_y = if dir.y != 0.0 { 1.0 / dir.y } else { f64::INFINITY };
    let inv_z = if dir.z != 0.0 { 1.0 / dir.z } else { f64::INFINITY };

    let t1 = (box_aabb.min.x - origin.x) * inv_x;
    let t2 = (box_aabb.max.x - origin.x) * inv_x;
    let t3 = (box_aabb.min.y - origin.y) * inv_y;
    let t4 = (box_aabb.max.y - origin.y) * inv_y;
    let t5 = (box_aabb.min.z - origin.z) * inv_z;
    let t6 = (box_aabb.max.z - origin.z) * inv_z;

    let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
    let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

    if tmax < 0.0 || tmin > tmax || tmin > max_reach {
        None
    } else {
        Some(tmin.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{CanonicalBlockState, ChunkColumn};

    #[test]
    fn test_dda_raycast_direct_hit() {
        let mut world = ClientWorldCache::new();
        let mut chunk = ChunkColumn::new(0, 0, 0, 16);
        let stone = CanonicalBlockState::new(1, 0);
        chunk.set_block(0, 64, 5, stone); // Khoi tai (0, 64, 5)
        world.insert_chunk(chunk);

        // Ban tia tu (0.5, 64.5, 0.0) nhin theo truc Z+ huong ve phia (0, 64, 5)
        let origin = DVec3::new(0.5, 64.5, 0.0);
        let dir = DVec3::new(0.0, 0.0, 1.0);
        let hit = raycast_block(&world, origin, dir, 10.0).expect("Phai tim thay khoi da");

        assert_eq!(hit.block_pos, IVec3::new(0, 64, 5));
        assert_eq!(hit.face, BlockFace::North); // Nhin vao mat Bac cua khoi (Z-)
        assert!((hit.distance - 5.0).abs() < 1e-4);
    }

    #[test]
    fn test_ray_intersects_aabb_pvp_hitbox() {
        // Gia lap Hitbox cua doi thu tai (0, 64, 3) rong 0.6 cao 1.8
        let opponent_box = Aabb::new(
            DVec3::new(-0.3, 64.0, 2.7),
            DVec3::new(0.3, 65.8, 3.3),
        );

        let eye_pos = DVec3::new(0.0, 65.0, 0.0);
        let attack_dir = DVec3::new(0.0, 0.0, 1.0);

        let dist = ray_intersects_aabb(eye_pos, attack_dir, &opponent_box, 3.5);
        assert!(dist.is_some());
        let d = dist.unwrap();
        assert!((d - 2.7).abs() < 1e-4);

        // Ban chech huong
        let miss_dir = DVec3::new(1.0, 0.0, 0.0);
        assert!(ray_intersects_aabb(eye_pos, miss_dir, &opponent_box, 3.5).is_none());
    }
}
