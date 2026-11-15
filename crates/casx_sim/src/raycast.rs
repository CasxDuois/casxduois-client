// crates/casx_sim/src/raycast.rs
// Fast 3D DDA voxel traversal algorithm (Amanatides & Woo).

use casx_core::jmath::{DVec3, IVec3};
use casx_world::WorldStorage;

/// Block face intersected by ray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFace {
    Bottom = 0,
    Top = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl BlockFace {
    pub fn normal(&self) -> IVec3 {
        match self {
            Self::Bottom => IVec3::new(0, -1, 0),
            Self::Top => IVec3::new(0, 1, 0),
            Self::North => IVec3::new(0, 0, -1),
            Self::South => IVec3::new(0, 0, 1),
            Self::West => IVec3::new(-1, 0, 0),
            Self::East => IVec3::new(1, 0, 0),
        }
    }
}

/// Raycast collision hit result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelHit {
    pub block_pos: IVec3,
    pub face: BlockFace,
    pub hit_point: DVec3,
    pub adjacent_pos: IVec3,
    pub distance: f64,
}

/// Casts a ray from eye position up to max_distance to find targeted block.
pub fn raycast_world(
    world: &WorldStorage,
    origin: DVec3,
    direction: DVec3,
    max_distance: f64,
) -> Option<VoxelHit> {
    let dir_len = direction.length();
    if dir_len < 1e-6 {
        return None;
    }
    let dir = direction * (1.0 / dir_len);

    let mut current_block = origin.floor_to_ivec3();

    // Check if ray origin starts inside a solid block
    if world.is_solid_block(current_block.x, current_block.y, current_block.z) {
        return Some(VoxelHit {
            block_pos: current_block,
            face: BlockFace::Top,
            hit_point: origin,
            adjacent_pos: current_block,
            distance: 0.0,
        });
    }

    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };

    let t_delta_x = if dir.x != 0.0 {
        (1.0 / dir.x).abs()
    } else {
        f64::INFINITY
    };
    let t_delta_y = if dir.y != 0.0 {
        (1.0 / dir.y).abs()
    } else {
        f64::INFINITY
    };
    let t_delta_z = if dir.z != 0.0 {
        (1.0 / dir.z).abs()
    } else {
        f64::INFINITY
    };

    let mut t_max_x = if dir.x > 0.0 {
        ((current_block.x + 1) as f64 - origin.x) * t_delta_x
    } else if dir.x < 0.0 {
        (origin.x - current_block.x as f64) * t_delta_x
    } else {
        f64::INFINITY
    };

    let mut t_max_y = if dir.y > 0.0 {
        ((current_block.y + 1) as f64 - origin.y) * t_delta_y
    } else if dir.y < 0.0 {
        (origin.y - current_block.y as f64) * t_delta_y
    } else {
        f64::INFINITY
    };

    let mut t_max_z = if dir.z > 0.0 {
        ((current_block.z + 1) as f64 - origin.z) * t_delta_z
    } else if dir.z < 0.0 {
        (origin.z - current_block.z as f64) * t_delta_z
    } else {
        f64::INFINITY
    };

    let mut hit_face;
    let mut total_dist = 0.0;

    while total_dist <= max_distance {
        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                current_block.x += step_x;
                total_dist = t_max_x;
                t_max_x += t_delta_x;
                hit_face = if step_x > 0 {
                    BlockFace::West
                } else {
                    BlockFace::East
                };
            } else {
                current_block.z += step_z;
                total_dist = t_max_z;
                t_max_z += t_delta_z;
                hit_face = if step_z > 0 {
                    BlockFace::North
                } else {
                    BlockFace::South
                };
            }
        } else {
            if t_max_y < t_max_z {
                current_block.y += step_y;
                total_dist = t_max_y;
                t_max_y += t_delta_y;
                hit_face = if step_y > 0 {
                    BlockFace::Bottom
                } else {
                    BlockFace::Top
                };
            } else {
                current_block.z += step_z;
                total_dist = t_max_z;
                t_max_z += t_delta_z;
                hit_face = if step_z > 0 {
                    BlockFace::North
                } else {
                    BlockFace::South
                };
            }
        }

        if total_dist > max_distance {
            break;
        }

        if world.is_solid_block(current_block.x, current_block.y, current_block.z) {
            let hit_point = origin + dir * total_dist;
            let norm = hit_face.normal();
            let adjacent_pos = IVec3::new(
                current_block.x + norm.x,
                current_block.y + norm.y,
                current_block.z + norm.z,
            );
            return Some(VoxelHit {
                block_pos: current_block,
                face: hit_face,
                hit_point,
                adjacent_pos,
                distance: total_dist,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use casx_core::palette::SectionPaletteV340;
    use casx_world::region::{ChunkColumnV340, ChunkSectionV340};

    #[test]
    fn test_raycast_direct_hit_top_face() {
        let mut world = WorldStorage::new();
        // Place stone block at (0, 64, 0)
        let mut data = vec![0u64; 256];
        data[0] |= 1u64;
        let palette = vec![0, 16];
        let section = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        let mut col = ChunkColumnV340::new(0, 0);
        col.sections.insert(
            4,
            ChunkSectionV340::new_v340(4, section, vec![15; 2048], None),
        );
        world.chunks.insert((0, 0), col);

        let origin = DVec3::new(0.5, 66.0, 0.5);
        let direction = DVec3::new(0.0, -1.0, 0.0);
        let hit = raycast_world(&world, origin, direction, 5.0).unwrap();

        assert_eq!(hit.block_pos, IVec3::new(0, 64, 0));
        assert_eq!(hit.face, BlockFace::Top);
        assert_eq!(hit.adjacent_pos, IVec3::new(0, 65, 0));
        assert!((hit.distance - 1.0).abs() < 1e-4);
    }
}
