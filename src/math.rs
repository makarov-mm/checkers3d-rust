//! Small vector and matrix types. Matrices are column-major, like OpenGL's, and multiply column vectors.

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

pub const PI: f32 = std::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

pub const fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

pub const fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vec4 {
    Vec4 { x, y, z, w }
}

impl Vec3 {
    pub const ZERO: Vec3 = vec3(0.0, 0.0, 0.0);
    pub const X: Vec3 = vec3(1.0, 0.0, 0.0);
    pub const Y: Vec3 = vec3(0.0, 1.0, 0.0);

    pub fn dot(self, other: Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        vec3(self.y * other.z - self.z * other.y, self.z * other.x - self.x * other.z, self.x * other.y - self.y * other.x)
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// The vector at unit length; a vector too short to have a direction points up.
    pub fn normalized(self) -> Vec3 {
        let length = self.length();
        if length > 1e-20 { self / length } else { Vec3::Y }
    }

    pub fn lerp(self, other: Vec3, t: f32) -> Vec3 {
        self + (other - self) * t
    }

    pub const fn extend(self, w: f32) -> Vec4 {
        vec4(self.x, self.y, self.z, w)
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, other: Vec3) -> Vec3 {
        vec3(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, other: Vec3) -> Vec3 {
        vec3(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        vec3(-self.x, -self.y, -self.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, scale: f32) -> Vec3 {
        vec3(self.x * scale, self.y * scale, self.z * scale)
    }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;
    fn mul(self, vector: Vec3) -> Vec3 {
        vector * self
    }
}

impl Mul for Vec3 {
    type Output = Vec3;
    fn mul(self, other: Vec3) -> Vec3 {
        vec3(self.x * other.x, self.y * other.y, self.z * other.z)
    }
}

impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, scale: f32) -> Vec3 {
        vec3(self.x / scale, self.y / scale, self.z / scale)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, other: Vec3) {
        *self = *self + other;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, other: Vec3) {
        *self = *self - other;
    }
}

impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, scale: f32) {
        *self = *self * scale;
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// A 4×4 matrix; element (row, column) is `m[column * 4 + row]`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Default for Mat4 {
    fn default() -> Mat4 {
        Mat4::IDENTITY
    }
}

impl Mat4 {
    pub const ZERO: Mat4 = Mat4 { m: [0.0; 16] };
    pub const IDENTITY: Mat4 = Mat4 { m: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0] };

    pub const fn at(&self, row: usize, column: usize) -> f32 {
        self.m[column * 4 + row]
    }

    pub const fn set(&mut self, row: usize, column: usize, value: f32) {
        self.m[column * 4 + row] = value;
    }

    pub const fn translation(t: Vec3) -> Mat4 {
        let mut result = Mat4::IDENTITY;
        result.set(0, 3, t.x);
        result.set(1, 3, t.y);
        result.set(2, 3, t.z);
        result
    }

    pub fn rotation_y(angle: f32) -> Mat4 {
        let (s, c) = angle.sin_cos();
        let mut result = Mat4::IDENTITY;
        result.set(0, 0, c);
        result.set(0, 2, s);
        result.set(2, 0, -s);
        result.set(2, 2, c);
        result
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let forward = (target - eye).normalized();
        let right = forward.cross(up).normalized();
        let true_up = right.cross(forward);
        let mut result = Mat4::IDENTITY;
        result.set(0, 0, right.x);
        result.set(0, 1, right.y);
        result.set(0, 2, right.z);
        result.set(1, 0, true_up.x);
        result.set(1, 1, true_up.y);
        result.set(1, 2, true_up.z);
        result.set(2, 0, -forward.x);
        result.set(2, 1, -forward.y);
        result.set(2, 2, -forward.z);
        result.set(0, 3, -right.dot(eye));
        result.set(1, 3, -true_up.dot(eye));
        result.set(2, 3, forward.dot(eye));
        result
    }

    /// OpenGL clip space: depth from -1 at the near plane to 1 at the far plane.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y / 2.0).tan();
        let mut result = Mat4::ZERO;
        result.set(0, 0, f / aspect);
        result.set(1, 1, f);
        result.set(2, 2, (far + near) / (near - far));
        result.set(2, 3, 2.0 * far * near / (near - far));
        result.set(3, 2, -1.0);
        result
    }

    pub const fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
        let mut result = Mat4::IDENTITY;
        result.set(0, 0, 2.0 / (right - left));
        result.set(1, 1, 2.0 / (top - bottom));
        result.set(2, 2, -2.0 / (far - near));
        result.set(0, 3, -(right + left) / (right - left));
        result.set(1, 3, -(top + bottom) / (top - bottom));
        result.set(2, 3, -(far + near) / (far - near));
        result
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let v = *self * p.extend(1.0);
        vec3(v.x, v.y, v.z) / v.w
    }

    pub fn inverse(&self) -> Mat4 {
        // Cofactors, after the expansion in Mesa's gluInvertMatrix.
        let a = &self.m;
        let mut inv = [0.0f32; 16];
        inv[0] = a[5] * a[10] * a[15] - a[5] * a[11] * a[14] - a[9] * a[6] * a[15] + a[9] * a[7] * a[14] + a[13] * a[6] * a[11] - a[13] * a[7] * a[10];
        inv[4] = -a[4] * a[10] * a[15] + a[4] * a[11] * a[14] + a[8] * a[6] * a[15] - a[8] * a[7] * a[14] - a[12] * a[6] * a[11] + a[12] * a[7] * a[10];
        inv[8] = a[4] * a[9] * a[15] - a[4] * a[11] * a[13] - a[8] * a[5] * a[15] + a[8] * a[7] * a[13] + a[12] * a[5] * a[11] - a[12] * a[7] * a[9];
        inv[12] = -a[4] * a[9] * a[14] + a[4] * a[10] * a[13] + a[8] * a[5] * a[14] - a[8] * a[6] * a[13] - a[12] * a[5] * a[10] + a[12] * a[6] * a[9];
        inv[1] = -a[1] * a[10] * a[15] + a[1] * a[11] * a[14] + a[9] * a[2] * a[15] - a[9] * a[3] * a[14] - a[13] * a[2] * a[11] + a[13] * a[3] * a[10];
        inv[5] = a[0] * a[10] * a[15] - a[0] * a[11] * a[14] - a[8] * a[2] * a[15] + a[8] * a[3] * a[14] + a[12] * a[2] * a[11] - a[12] * a[3] * a[10];
        inv[9] = -a[0] * a[9] * a[15] + a[0] * a[11] * a[13] + a[8] * a[1] * a[15] - a[8] * a[3] * a[13] - a[12] * a[1] * a[11] + a[12] * a[3] * a[9];
        inv[13] = a[0] * a[9] * a[14] - a[0] * a[10] * a[13] - a[8] * a[1] * a[14] + a[8] * a[2] * a[13] + a[12] * a[1] * a[10] - a[12] * a[2] * a[9];
        inv[2] = a[1] * a[6] * a[15] - a[1] * a[7] * a[14] - a[5] * a[2] * a[15] + a[5] * a[3] * a[14] + a[13] * a[2] * a[7] - a[13] * a[3] * a[6];
        inv[6] = -a[0] * a[6] * a[15] + a[0] * a[7] * a[14] + a[4] * a[2] * a[15] - a[4] * a[3] * a[14] - a[12] * a[2] * a[7] + a[12] * a[3] * a[6];
        inv[10] = a[0] * a[5] * a[15] - a[0] * a[7] * a[13] - a[4] * a[1] * a[15] + a[4] * a[3] * a[13] + a[12] * a[1] * a[7] - a[12] * a[3] * a[5];
        inv[14] = -a[0] * a[5] * a[14] + a[0] * a[6] * a[13] + a[4] * a[1] * a[14] - a[4] * a[2] * a[13] - a[12] * a[1] * a[6] + a[12] * a[2] * a[5];
        inv[3] = -a[1] * a[6] * a[11] + a[1] * a[7] * a[10] + a[5] * a[2] * a[11] - a[5] * a[3] * a[10] - a[9] * a[2] * a[7] + a[9] * a[3] * a[6];
        inv[7] = a[0] * a[6] * a[11] - a[0] * a[7] * a[10] - a[4] * a[2] * a[11] + a[4] * a[3] * a[10] + a[8] * a[2] * a[7] - a[8] * a[3] * a[6];
        inv[11] = -a[0] * a[5] * a[11] + a[0] * a[7] * a[9] + a[4] * a[1] * a[11] - a[4] * a[3] * a[9] - a[8] * a[1] * a[7] + a[8] * a[3] * a[5];
        inv[15] = a[0] * a[5] * a[10] - a[0] * a[6] * a[9] - a[4] * a[1] * a[10] + a[4] * a[2] * a[9] + a[8] * a[1] * a[6] - a[8] * a[2] * a[5];

        let determinant = a[0] * inv[0] + a[1] * inv[4] + a[2] * inv[8] + a[3] * inv[12];
        if determinant == 0.0 {
            return Mat4::IDENTITY;
        }
        Mat4 { m: inv.map(|value| value / determinant) }
    }
}

impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, other: Mat4) -> Mat4 {
        let mut result = Mat4::ZERO;
        for row in 0..4 {
            for column in 0..4 {
                let sum = (0..4).map(|k| self.at(row, k) * other.at(k, column)).sum();
                result.set(row, column, sum);
            }
        }
        result
    }
}

impl Mul<Vec4> for Mat4 {
    type Output = Vec4;
    fn mul(self, v: Vec4) -> Vec4 {
        let row = |r| self.at(r, 0) * v.x + self.at(r, 1) * v.y + self.at(r, 2) * v.z + self.at(r, 3) * v.w;
        vec4(row(0), row(1), row(2), row(3))
    }
}
