//! The pieces: a draughtsman turned on a lathe, with a groove round its side and rings on its top.
//! A king is two of them, one on the other.

use crate::math::{PI, Vec3, lerp, smooth_step, vec3};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub occlusion: f32, // light reaching into the grooves
}

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

// A square of the board is 1 wide.
pub const RADIUS: f32 = 0.4;
pub const HEIGHT: f32 = 0.15;

pub struct DiscShape {
    pub detailed: Mesh, // for the scene
    pub simple: Mesh,   // for the shadows and the reflections
}

struct ProfilePoint {
    r: f32,
    y: f32,
    occlusion: f32,
}

const EDGE_RADIUS: f32 = 0.026; // the rounded lower edge
const RIM_RADIUS: f32 = 0.032; // the rounded upper edge
const RECESS: f32 = 0.245; // the radius of the dished middle of the top

/// A smooth dip of width 2 * half_width: 1 in the middle, 0 at the edges.
fn bump(x: f32, center: f32, half_width: f32) -> f32 {
    let t = (x - center) / half_width;
    if t.abs() < 1.0 { (1.0 - t * t) * (1.0 - t * t) } else { 0.0 }
}

/// The height of the top at a distance from the axis: a groove near the rim, a step down into a shallow
/// dish, and a fine ring in it.
fn top_height(r: f32) -> f32 {
    let mut y = HEIGHT;
    y -= 0.007 * bump(r, 0.305, 0.010);
    y -= 0.010 * smooth_step(RECESS + 0.016, RECESS, r);
    if r < RECESS {
        y -= 0.006 * (1.0 - (r / RECESS) * (r / RECESS));
    }
    y - 0.0028 * bump(r, 0.155, 0.007)
}

fn top_occlusion(r: f32) -> f32 {
    1.0 - 0.45 * bump(r, 0.305, 0.012) - 0.3 * bump(r, RECESS + 0.004, 0.012) - 0.25 * bump(r, 0.155, 0.008)
}

/// The side, between the two rounded edges, with a groove near the top and one near the bottom.
fn side_radius(y: f32) -> f32 {
    RADIUS - 0.005 * bump(y, 0.046, 0.008) - 0.005 * bump(y, HEIGHT - 0.052, 0.008)
}

fn side_occlusion(y: f32) -> f32 {
    1.0 - 0.35 * bump(y, 0.046, 0.009) - 0.35 * bump(y, HEIGHT - 0.052, 0.009)
}

/// The outline from the middle of the bottom, round the edge and over the top to the middle of the top.
fn profile(detail: usize) -> Vec<ProfilePoint> {
    let count = |n: usize| (n * detail / 4).max(2);
    let fraction = |i: usize, n: usize| i as f32 / n as f32;
    let mut points = Vec::new();

    // The bottom, covered with cloth.
    let bottom = count(4);
    points.extend((0..bottom).map(|i| ProfilePoint { r: (RADIUS - EDGE_RADIUS) * fraction(i, bottom), y: 0.0, occlusion: 1.0 }));

    // The lower edge.
    let edge = count(8);
    points.extend((0..edge).map(|i| {
        let (sin, cos) = (-PI / 2.0 + PI / 2.0 * fraction(i, edge)).sin_cos();
        ProfilePoint { r: RADIUS - EDGE_RADIUS + EDGE_RADIUS * cos, y: EDGE_RADIUS + EDGE_RADIUS * sin, occlusion: 1.0 }
    }));

    // The side.
    let side = count(28);
    points.extend((0..side).map(|i| {
        let y = lerp(EDGE_RADIUS, HEIGHT - RIM_RADIUS, fraction(i, side));
        ProfilePoint { r: side_radius(y), y, occlusion: side_occlusion(y) }
    }));

    // The upper edge.
    let rim = count(10);
    points.extend((0..rim).map(|i| {
        let (sin, cos) = (PI / 2.0 * fraction(i, rim)).sin_cos();
        ProfilePoint { r: RADIUS - RIM_RADIUS + RIM_RADIUS * cos, y: HEIGHT - RIM_RADIUS + RIM_RADIUS * sin, occlusion: 1.0 }
    }));

    // The top, inwards.
    let top = count(90);
    points.extend((0..=top).map(|i| {
        let r = (RADIUS - RIM_RADIUS) * (1.0 - fraction(i, top));
        ProfilePoint { r, y: top_height(r), occlusion: top_occlusion(r) }
    }));
    points
}

/// The profile turned round the vertical axis. Normals come from the neighbouring points of the outline.
fn lathe(profile: &[ProfilePoint], segments: u32) -> Mesh {
    let count = profile.len();
    let mut normals: Vec<(f32, f32)> = (0..count)
        .map(|i| {
            let (before, after) = (&profile[i.saturating_sub(1)], &profile[(i + 1).min(count - 1)]);
            let (dr, dy) = (after.r - before.r, after.y - before.y);
            let length = dr.hypot(dy);
            if length > 0.0 { (dy / length, -dr / length) } else { (0.0, 1.0) }
        })
        .collect();
    // The middles of the bottom and of the top face straight down and up.
    normals[0] = (0.0, -1.0);
    normals[count - 1] = (0.0, 1.0);

    let mut mesh = Mesh::default();
    for (point, normal) in profile.iter().zip(&normals) {
        for j in 0..=segments {
            let (s, c) = (2.0 * PI * j as f32 / segments as f32).sin_cos();
            mesh.vertices.push(Vertex {
                position: vec3(point.r * c, point.y, point.r * s),
                normal: vec3(normal.0 * c, normal.1, normal.0 * s).normalized(),
                occlusion: point.occlusion,
            });
        }
    }

    let index = |i: usize, j: u32| i as u32 * (segments + 1) + j;
    for i in 0..count - 1 {
        for j in 0..segments {
            // Next to the axis a quad narrows to a triangle.
            if profile[i].r > 0.0 {
                mesh.indices.extend([index(i, j), index(i + 1, j + 1), index(i, j + 1)]);
            }
            if profile[i + 1].r > 0.0 {
                mesh.indices.extend([index(i, j), index(i + 1, j), index(i + 1, j + 1)]);
            }
        }
    }
    mesh
}

pub fn build_disc() -> DiscShape {
    DiscShape { detailed: lathe(&profile(4), 96), simple: lathe(&profile(1), 32) }
}
