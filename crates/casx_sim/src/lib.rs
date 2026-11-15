//! Crate `casx-sim`: Fixed 20 TPS client physics, AABB collision resolution, and inventory state.

pub mod aabb;
pub mod raycast;

pub use aabb::{move_entity_with_collisions, PlayerSimulation};
pub use raycast::{raycast_world, BlockFace, VoxelHit};
