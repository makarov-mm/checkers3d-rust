//! A camera circling the board. Its moves are eased: dragging, zooming and turning set where it heads.

use crate::draughts::Color;
use crate::math::{Mat4, PI, Vec3, vec3};

const DEFAULT_PITCH: f32 = 0.84;
const DEFAULT_DISTANCE: f32 = 15.5;
const FIELD_OF_VIEW: f32 = 0.6;
const MIN_ASPECT: f32 = 1.4; // of the space for the board, below which the view widens
const MIN_PITCH: f32 = 0.22;
const MAX_PITCH: f32 = 1.53;
const MIN_DISTANCE: f32 = 8.0;
const MAX_DISTANCE: f32 = 26.0;
const TARGET: Vec3 = vec3(0.0, 0.15, 0.0);

/// The angle nearest to `current` that points the same way as `angle`.
fn unwrap(mut angle: f32, current: f32) -> f32 {
    while angle - current > PI {
        angle -= 2.0 * PI;
    }
    while angle - current < -PI {
        angle += 2.0 * PI;
    }
    angle
}

/// Where the camera is, or where it heads.
#[derive(Clone, Copy)]
struct Pose {
    yaw: f32,
    pitch: f32,
    distance: f32,
    shift: f32,
    space: f32,
}

pub struct OrbitCamera {
    now: Pose,
    target: Pose,
    top_view: bool,
    saved_pitch: f32,
}

impl OrbitCamera {
    pub fn new() -> OrbitCamera {
        let pose = Pose { yaw: 0.0, pitch: DEFAULT_PITCH, distance: DEFAULT_DISTANCE, shift: 0.0, space: 1.0 };
        OrbitCamera { now: pose, target: pose, top_view: false, saved_pitch: DEFAULT_PITCH }
    }

    /// White looks from +z, where rank 1 is near; Black from the other side.
    pub fn face_side(&mut self, side: Color) {
        self.target.yaw = unwrap(if side == Color::White { 0.0 } else { PI }, self.target.yaw);
    }

    pub fn flip(&mut self) {
        self.target.yaw += PI;
    }

    pub fn toggle_top_view(&mut self) {
        self.top_view = !self.top_view;
        if self.top_view {
            self.saved_pitch = self.target.pitch;
            self.target.pitch = MAX_PITCH;
        } else {
            self.target.pitch = self.saved_pitch;
        }
    }

    pub fn reset(&mut self, side: Color) {
        self.face_side(side);
        self.target.pitch = DEFAULT_PITCH;
        self.target.distance = DEFAULT_DISTANCE;
        self.top_view = false;
    }

    pub fn rotate(&mut self, yaw: f32, pitch: f32) {
        self.target.yaw += yaw;
        self.target.pitch = (self.target.pitch + pitch).clamp(MIN_PITCH, MAX_PITCH);
        self.top_view = false;
    }

    pub fn zoom(&mut self, steps: f32) {
        self.target.distance = (self.target.distance * 0.9f32.powf(steps)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// The share of the window's width left for the board beside the panel: the view centers on it, and
    /// widens when it is narrow, so the board and the pieces taken stay in sight.
    pub fn set_space(&mut self, share: f32) {
        self.target.shift = (share - 1.0) / 2.0;
        self.target.space = share;
    }

    pub fn update(&mut self, seconds: f32) {
        let ease = 1.0 - (-seconds * 9.0).exp();
        self.now.yaw += (self.target.yaw - self.now.yaw) * ease;
        self.now.pitch += (self.target.pitch - self.now.pitch) * ease;
        self.now.distance += (self.target.distance - self.now.distance) * ease;
        self.now.shift += (self.target.shift - self.now.shift) * ease;
        self.now.space += (self.target.space - self.now.space) * ease;
    }

    pub fn moving(&self) -> bool {
        (self.target.yaw - self.now.yaw).abs() > 1e-4
            || (self.target.pitch - self.now.pitch).abs() > 1e-4
            || (self.target.distance - self.now.distance).abs() > 1e-3
            || (self.target.shift - self.now.shift).abs() > 1e-4
            || (self.target.space - self.now.space).abs() > 1e-4
    }

    pub fn eye(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.now.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.now.pitch.sin_cos();
        TARGET + vec3(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos) * self.now.distance
    }

    pub fn view(&self) -> Mat4 {
        // Looking straight down, "up" on the screen points away from the player.
        let (yaw_sin, yaw_cos) = self.now.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.now.pitch.sin_cos();
        let up = vec3(-yaw_sin * pitch_sin, pitch_cos, -yaw_cos * pitch_sin).normalized();
        Mat4::look_at(self.eye(), TARGET, up)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        let widen = (MIN_ASPECT / (aspect * self.now.space)).max(1.0);
        let mut projection = Mat4::perspective(2.0 * ((FIELD_OF_VIEW / 2.0).tan() * widen).atan(), aspect, 0.2, 120.0);
        projection.set(0, 2, -2.0 * self.now.shift); // shift in normalized coordinates, which span 2
        projection
    }

    /// The ray through a point of the window, in pixels from the top left: its origin and its direction.
    pub fn ray(&self, x: f32, y: f32, width: f32, height: f32) -> (Vec3, Vec3) {
        let inverse = (self.projection(width / height.max(1.0)) * self.view()).inverse();
        let (nx, ny) = (x / width * 2.0 - 1.0, 1.0 - y / height * 2.0);
        let near = inverse.transform_point(vec3(nx, ny, -1.0));
        let far = inverse.transform_point(vec3(nx, ny, 1.0));
        (near, (far - near).normalized())
    }
}
