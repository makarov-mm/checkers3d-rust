//! The board, its frame, the table and the hint arrow. A square is one unit; the playing surface is the
//! plane y = 0, a1 is at the left front corner seen from White, and White sits on the +z side.

use crate::draughts::{self, Square};
use crate::math::{PI, Vec2, Vec3, vec2, vec3};

pub const FRAME_INNER: f32 = 4.0;
pub const FRAME_OUTER: f32 = 4.62;
pub const FRAME_TOP: f32 = 0.03;
pub const TABLE_HEIGHT: f32 = -0.32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub fn square_center(square: Square) -> Vec3 {
    vec3(draughts::file_of(square) as f32 - 3.5, 0.0, 3.5 - draughts::rank_of(square) as f32)
}

/// A square's bit among the squares counted over the whole board, light ones too: file + 8 * rank. The marks
/// on the board use these.
pub fn board_bit(square: Square) -> u64 {
    1 << (draughts::file_of(square) + 8 * draughts::rank_of(square))
}

/// The dark square under a point of the playing surface, if it is one.
pub fn square_at(point: Vec3) -> Option<Square> {
    let square = draughts::square_at((point.x + 4.0).floor() as i32, (4.0 - point.z).floor() as i32);
    (square >= 0).then_some(square)
}

/// Adds a quad, wound counterclockwise as seen from the side its normal faces.
fn add_quad(mesh: &mut Mesh, corners: [Vertex; 4]) {
    let base = mesh.vertices.len() as u32;
    let [a, b, c, d] = corners;
    let facing = (b.position - a.position).cross(c.position - a.position);
    let normal = a.normal + b.normal + c.normal + d.normal;
    mesh.vertices.extend(corners);
    if facing.dot(normal) >= 0.0 {
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        mesh.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

pub fn build_squares() -> Mesh {
    let mut mesh = Mesh::default();
    let vertex = |x, z, u, v| Vertex { position: vec3(x, 0.0, z), normal: Vec3::Y, uv: vec2(u, v) };
    add_quad(&mut mesh, [vertex(-4.0, 4.0, 0.0, 0.0), vertex(4.0, 4.0, 1.0, 0.0), vertex(4.0, -4.0, 1.0, 1.0), vertex(-4.0, -4.0, 0.0, 1.0)]);
    mesh
}

/// The frame's cross-section, swept round the board with mitred corners: a rounded inner edge rising from
/// the squares, a flat top with an eased outer edge, and the side down to the table. The inner edge is a
/// quarter circle rather than a step, so that it does not shrink to a flickering sliver when seen from afar.
pub fn build_frame() -> Mesh {
    struct ProfilePoint {
        // Outward from the playing area's edge, and height.
        out: f32,
        y: f32,
        // The normal.
        n_out: f32,
        n_y: f32,
    }

    const fn point(out: f32, y: f32, n_out: f32, n_y: f32) -> ProfilePoint {
        ProfilePoint { out, y, n_out, n_y }
    }

    const PROFILE: [ProfilePoint; 11] = [
        point(0.000, -0.002, -1.0, 0.0),
        point(0.000, 0.000, -1.0, 0.0),
        point(0.0023, 0.0115, -0.924, 0.383),
        point(0.0088, 0.0212, -0.707, 0.707),
        point(0.0185, 0.0277, -0.383, 0.924),
        point(0.030, FRAME_TOP, 0.0, 1.0),
        point(0.585, FRAME_TOP, 0.0, 1.0),
        point(0.607, 0.022, 0.62, 0.78),
        point(0.618, 0.006, 0.96, 0.28),
        point(0.62, -0.012, 1.0, 0.0),
        point(0.62, TABLE_HEIGHT, 1.0, 0.0),
    ];

    const OUTWARD: [Vec3; 4] = [vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec3(-1.0, 0.0, 0.0)];

    let mut mesh = Mesh::default();
    for o in OUTWARD {
        let along = vec3(-o.z, 0.0, o.x); // along the side
        for pair in PROFILE.windows(2) {
            let (p, q) = (&pair[0], &pair[1]);
            if p.out == q.out && p.y == q.y {
                continue;
            }
            let (p_extent, q_extent) = (FRAME_INNER + p.out, FRAME_INNER + q.out);
            let vertex = |point: &ProfilePoint, extent: f32, s: f32| Vertex {
                position: o * extent + along * s + vec3(0.0, point.y, 0.0),
                normal: (o * point.n_out + vec3(0.0, point.n_y, 0.0)).normalized(),
                uv: vec2(s, point.out),
            };
            add_quad(&mut mesh, [vertex(p, p_extent, -p_extent), vertex(p, p_extent, p_extent), vertex(q, q_extent, q_extent), vertex(q, q_extent, -q_extent)]);
        }
    }
    mesh
}

pub fn build_table(radius: f32) -> Mesh {
    const SEGMENTS: u32 = 96;
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex { position: vec3(0.0, TABLE_HEIGHT, 0.0), normal: Vec3::Y, uv: vec2(0.5, 0.5) });
    for i in 0..SEGMENTS {
        let (s, c) = (2.0 * PI * i as f32 / SEGMENTS as f32).sin_cos();
        mesh.vertices.push(Vertex { position: vec3(c * radius, TABLE_HEIGHT, s * radius), normal: Vec3::Y, uv: vec2(c * 0.5 + 0.5, s * 0.5 + 0.5) });
    }
    for i in 0..SEGMENTS {
        // Counterclockwise from above: from +x toward -z.
        let (a, b) = (1 + i, 1 + (i + 1) % SEGMENTS);
        mesh.indices.extend([0, b, a]);
    }
    mesh
}

/// An arrow lying on the board along a route of square centers, with its head at the last.
pub fn build_arrow(route: &[Vec3]) -> Mesh {
    const LIFT: Vec3 = vec3(0.0, 0.012, 0.0);
    const SHAFT_HALF: f32 = 0.085;
    const HEAD_HALF: f32 = 0.24;
    const HEAD_LENGTH: f32 = 0.42;

    let mut mesh = Mesh::default();
    if route.len() < 2 {
        return mesh;
    }

    // The route, starting clear of the piece and stopping short of the last square's middle.
    let mut points: Vec<Vec3> = route.iter().map(|point| vec3(point.x, 0.0, point.z)).collect();
    let last = points.len() - 1;
    let first_direction = (points[1] - points[0]).normalized();
    points[0] += first_direction * 0.28;
    let last_direction = (points[last] - points[last - 1]).normalized();
    let tip = points[last] - last_direction * 0.12;
    points[last] = tip - last_direction * HEAD_LENGTH; // the neck, where the head begins
    let total = points.windows(2).map(|pair| (pair[1] - pair[0]).length()).sum::<f32>() + HEAD_LENGTH;

    let mut triangle = |a: Vertex, b: Vertex, c: Vertex| {
        let base = mesh.vertices.len() as u32;
        // Counterclockwise from above.
        let upward = (b.position - a.position).cross(c.position - a.position).y > 0.0;
        mesh.vertices.extend([a, b, c]);
        mesh.indices.extend(if upward { [base, base + 1, base + 2] } else { [base, base + 2, base + 1] });
    };

    // The shaft: a ribbon, mitred at the turns.
    let mut travelled = 0.0;
    let mut edges: Vec<(Vertex, Vertex)> = Vec::new();
    for i in 0..points.len() {
        let incoming = if i > 0 { points[i] - points[i - 1] } else { points[1] - points[0] }.normalized();
        let outgoing = if i < last { (points[i + 1] - points[i]).normalized() } else { incoming };
        let sum = incoming + outgoing;
        let direction = if sum.length() > 1e-4 { sum.normalized() } else { outgoing };
        let side = vec3(-direction.z, 0.0, direction.x);
        let side_in = vec3(-incoming.z, 0.0, incoming.x);
        let miter = (1.0 / side.dot(side_in).max(0.3)).min(2.5);
        if i > 0 {
            travelled += (points[i] - points[i - 1]).length();
        }
        let u = travelled / total;
        let offset = side * (SHAFT_HALF * miter);
        edges.push((
            Vertex { position: points[i] + LIFT - offset, normal: Vec3::Y, uv: vec2(u, -1.0) },
            Vertex { position: points[i] + LIFT + offset, normal: Vec3::Y, uv: vec2(u, 1.0) },
        ));
    }
    for pair in edges.windows(2) {
        let ((left, right), (next_left, next_right)) = (pair[0], pair[1]);
        triangle(left, right, next_right);
        triangle(left, next_right, next_left);
    }

    // The head.
    let side = vec3(-last_direction.z, 0.0, last_direction.x);
    let neck_u = travelled / total;
    triangle(
        Vertex { position: points[last] + LIFT - side * HEAD_HALF, normal: Vec3::Y, uv: vec2(neck_u, -1.0) },
        Vertex { position: points[last] + LIFT + side * HEAD_HALF, normal: Vec3::Y, uv: vec2(neck_u, 1.0) },
        Vertex { position: tip + LIFT, normal: Vec3::Y, uv: vec2(1.0, 0.0) },
    );
    mesh
}
