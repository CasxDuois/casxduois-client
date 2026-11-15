//! Crate `casx-sim`: Vong lap vat ly 20 TPS, va cham AABB Swept, do tia 3D DDA va tiep dat tren dia hinh Voxel.

pub mod aabb;
pub mod raycast;

pub use aabb::{move_entity_with_collisions, PlayerSimulation};
pub use raycast::{raycast_world, BlockFace, VoxelHit};
