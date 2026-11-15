//! 6-plane View-Frustum Culling to minimize GPU draw calls.

use casx_core::physics::Aabb;
use glam::{Mat4, Vec3, Vec4};

/// 3D space plane represented by equation $Ax + By + Cz + D = 0$
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    pub fn from_vec4(v: Vec4) -> Self {
        let normal = Vec3::new(v.x, v.y, v.z);
        let len = normal.length();
        if len > 1e-6 {
            let inv_len = 1.0 / len;
            Self {
                normal: normal * inv_len,
                d: v.w * inv_len,
            }
        } else {
            Self {
                normal: Vec3::Y,
                d: 0.0,
            }
        }
    }

    /// Computes signed distance from a point to the plane
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }
}

/// View frustum containing 6 bounding planes (Left, Right, Bottom, Top, Near, Far)
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Extracts 6 frustum planes from View-Projection matrix
    pub fn from_view_projection(vp: Mat4) -> Self {
        let cols = vp.to_cols_array_2d();
        let r0 = Vec4::new(cols[0][0], cols[1][0], cols[2][0], cols[3][0]);
        let r1 = Vec4::new(cols[0][1], cols[1][1], cols[2][1], cols[3][1]);
        let r2 = Vec4::new(cols[0][2], cols[1][2], cols[2][2], cols[3][2]);
        let r3 = Vec4::new(cols[0][3], cols[1][3], cols[2][3], cols[3][3]);

        let planes = [
            Plane::from_vec4(r3 + r0), // Left
            Plane::from_vec4(r3 - r0), // Right
            Plane::from_vec4(r3 + r1), // Bottom
            Plane::from_vec4(r3 - r1), // Top
            Plane::from_vec4(r2),      // Near (wgpu [0, 1] depth)
            Plane::from_vec4(r3 - r2), // Far
        ];

        Self { planes }
    }

    /// Checks if an AABB bounding box is inside or intersects the frustum.
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        let min = Vec3::new(aabb.min.x as f32, aabb.min.y as f32, aabb.min.z as f32);
        let max = Vec3::new(aabb.max.x as f32, aabb.max.y as f32, aabb.max.z as f32);

        for plane in &self.planes {
            // Select the positive vertex along plane normal
            let p = Vec3::new(
                if plane.normal.x > 0.0 { max.x } else { min.x },
                if plane.normal.y > 0.0 { max.y } else { min.y },
                if plane.normal.z > 0.0 { max.z } else { min.z },
            );

            if plane.distance_to_point(p) < 0.0 {
                // Entire box is outside this plane
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    #[test]
    fn test_frustum_culling_inside_and_outside() {
        let eye = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(0.0, 0.0, -10.0);
        let up = Vec3::Y;
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh(60.0f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        let vp = proj * view;

        let frustum = Frustum::from_view_projection(vp);

        // Box directly in front of camera
        let inside_box = Aabb::new(DVec3::new(-1.0, -1.0, -20.0), DVec3::new(1.0, 1.0, -18.0));
        assert!(
            frustum.intersects_aabb(&inside_box),
            "Front box must be inside Frustum"
        );

        // Box behind camera
        let behind_box = Aabb::new(DVec3::new(-1.0, -1.0, 5.0), DVec3::new(1.0, 1.0, 10.0));
        assert!(
            !frustum.intersects_aabb(&behind_box),
            "Behind box must be culled by Frustum"
        );
    }
}
