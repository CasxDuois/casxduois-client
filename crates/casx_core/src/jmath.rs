// crates/casx_core/src/jmath.rs
// 64-bit vector and AABB bounding box math utilities for Minecraft Engine

use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

/// 3-dimensional double-precision floating point vector (64-bit float)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl DVec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn length_squared(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline]
    pub fn length(&self) -> f64 {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn distance(&self, other: &Self) -> f64 {
        (*self - *other).length()
    }

    #[inline]
    pub fn dot(&self, other: &Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn floor_to_ivec3(&self) -> IVec3 {
        IVec3::new(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.z.floor() as i32,
        )
    }

    #[inline]
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }
}

impl Add for DVec3 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl AddAssign for DVec3 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for DVec3 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for DVec3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul<f64> for DVec3 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

/// 32-bit signed integer 3D vector (Block/Chunk coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IVec3 {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn to_dvec3(&self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

/// Axis-Aligned Bounding Box (AABB) using 64-bit floating point precision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: DVec3,
    pub max: DVec3,
}

impl AABB {
    #[inline]
    pub fn new(min: DVec3, max: DVec3) -> Self {
        Self {
            min: DVec3::new(min.x.min(max.x), min.y.min(max.y), min.z.min(max.z)),
            max: DVec3::new(min.x.max(max.x), min.y.max(max.y), min.z.max(max.z)),
        }
    }

    /// Creates an AABB for a 1x1x1 solid block at integer grid coordinate pos.
    #[inline]
    pub fn from_block(pos: IVec3) -> Self {
        let min = pos.to_dvec3();
        let max = min + DVec3::new(1.0, 1.0, 1.0);
        Self { min, max }
    }

    /// Creates an AABB for a player entity given feet position (0.6x1.8x0.6).
    #[inline]
    pub fn player_box(feet_pos: DVec3) -> Self {
        let half_w = 0.3;
        let height = 1.8;
        Self {
            min: DVec3::new(feet_pos.x - half_w, feet_pos.y, feet_pos.z - half_w),
            max: DVec3::new(
                feet_pos.x + half_w,
                feet_pos.y + height,
                feet_pos.z + half_w,
            ),
        }
    }

    #[inline]
    pub fn offset(&self, d: DVec3) -> Self {
        Self {
            min: self.min + d,
            max: self.max + d,
        }
    }

    #[inline]
    pub fn intersects(&self, other: &Self) -> bool {
        self.max.x > other.min.x
            && self.min.x < other.max.x
            && self.max.y > other.min.y
            && self.min.y < other.max.y
            && self.max.z > other.min.z
            && self.min.z < other.max.z
    }

    /// Calculates allowed X displacement to prevent collision with obstacle `self`.
    pub fn calculate_x_offset(&self, moving: &Self, mut offset_x: f64) -> f64 {
        if moving.max.y > self.min.y
            && moving.min.y < self.max.y
            && moving.max.z > self.min.z
            && moving.min.z < self.max.z
        {
            if offset_x > 0.0 && moving.max.x <= self.min.x {
                let d = self.min.x - moving.max.x;
                if d < offset_x {
                    offset_x = d;
                }
            } else if offset_x < 0.0 && moving.min.x >= self.max.x {
                let d = self.max.x - moving.min.x;
                if d > offset_x {
                    offset_x = d;
                }
            }
        }
        offset_x
    }

    /// Calculates allowed Y displacement to prevent collision with obstacle `self`.
    pub fn calculate_y_offset(&self, moving: &Self, mut offset_y: f64) -> f64 {
        if moving.max.x > self.min.x
            && moving.min.x < self.max.x
            && moving.max.z > self.min.z
            && moving.min.z < self.max.z
        {
            if offset_y > 0.0 && moving.max.y <= self.min.y {
                let d = self.min.y - moving.max.y;
                if d < offset_y {
                    offset_y = d;
                }
            } else if offset_y < 0.0 && moving.min.y >= self.max.y {
                let d = self.max.y - moving.min.y;
                if d > offset_y {
                    offset_y = d;
                }
            }
        }
        offset_y
    }

    /// Calculates allowed Z displacement to prevent collision with obstacle `self`.
    pub fn calculate_z_offset(&self, moving: &Self, mut offset_z: f64) -> f64 {
        if moving.max.x > self.min.x
            && moving.min.x < self.max.x
            && moving.max.y > self.min.y
            && moving.min.y < self.max.y
        {
            if offset_z > 0.0 && moving.max.z <= self.min.z {
                let d = self.min.z - moving.max.z;
                if d < offset_z {
                    offset_z = d;
                }
            } else if offset_z < 0.0 && moving.min.z >= self.max.z {
                let d = self.max.z - moving.min.z;
                if d > offset_z {
                    offset_z = d;
                }
            }
        }
        offset_z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvec3_arithmetic_and_floor() {
        let v1 = DVec3::new(10.5, 64.2, -5.8);
        let v2 = DVec3::new(0.5, 0.8, 1.2);
        let sum = v1 + v2;
        assert_eq!(sum, DVec3::new(11.0, 65.0, -4.6));

        let block_pos = v1.floor_to_ivec3();
        assert_eq!(block_pos, IVec3::new(10, 64, -6));
    }

    #[test]
    fn test_aabb_vertical_collision_landing() {
        let floor = AABB::from_block(IVec3::new(0, 63, 0)); // y: 63.0 .. 64.0
        let player = AABB::player_box(DVec3::new(0.5, 64.5, 0.5)); // min.y = 64.5, max.y = 66.3

        // Floor la chuong ngai vat tinh, player roi xuong -1.0m
        let allowed_y = floor.calculate_y_offset(&player, -1.0);
        // Player min.y (64.5) roi toi floor max.y (64.0) -> chi duoc di -0.5m
        assert!((allowed_y - (-0.5)).abs() < 1e-6);

        let landed = player.offset(DVec3::new(0.0, allowed_y, 0.0));
        assert!((landed.min.y - 64.0).abs() < 1e-6);
    }
}
