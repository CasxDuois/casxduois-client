//! Crate `casx-sim`: Vong lap vat ly 20 TPS, va cham AABB Swept va tiep dat tren dia hinh Voxel.

pub mod aabb;

pub use aabb::{move_entity_with_collisions, PlayerSimulation};
