//! Draws the interface: rounded panels, shadows and text, in pixels from the window's top left corner.

use std::mem::{offset_of, size_of};

use crate::Result;
use crate::gfx::{self, Attribute, Buffer, Program, Stage, VertexArray};
use crate::math::{Vec3, Vec4, lerp, vec2, vec4};
use crate::shaders;
use crate::text::Font;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    rgba(r, g, b, 255)
}

impl Color {
    pub fn fade(self, opacity: f32) -> Color {
        Color { a: (f32::from(self.a) * opacity).clamp(0.0, 255.0) as u8, ..self }
    }

    pub fn mix(self, other: Color, t: f32) -> Color {
        let mix = |x: u8, y: u8| lerp(f32::from(x), f32::from(y), t.clamp(0.0, 1.0)).round() as u8;
        rgba(mix(self.r, other.r), mix(self.g, other.g), mix(self.b, other.b), mix(self.a, other.a))
    }

    /// A linear color of the scene, as it shows on the screen.
    pub fn from_linear(color: Vec3, alpha: f32) -> Color {
        let encode = |c: f32| (c.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
        rgba(encode(color.x), encode(color.y), encode(color.z), (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect { x, y, width, height }
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn inset(&self, d: f32) -> Rect {
        rect(self.x + d, self.y + d, self.width - 2.0 * d, self.height - 2.0 * d)
    }
}

#[derive(Clone, Copy)]
pub enum FontId {
    Body,
    Heading,
    Display,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    color: Color,
    shape: Vec4, // x: the kind; yzw depend on it
}

// The kinds of shape that the shader draws from their distance functions; the kinds below are the fonts.
const SHAPE_FILL: f32 = 3.0;
const SHAPE_SHADOW: f32 = 4.0;
const SHAPE_OUTLINE: f32 = 5.0;
const SHAPE_SPINNER: f32 = 6.0;
const SHAPE_DISC: f32 = 7.0;
const SHAPE_ARROW: f32 = 8.0;

pub struct Painter {
    program: Program,
    buffer: Buffer,
    vertex_array: VertexArray,
    vertices: Vec<Vertex>,
    fonts: Option<[Font; 3]>,
    width: i32,
    height: i32,
}

impl Painter {
    pub fn new() -> Result<Painter> {
        let program = Program::new(
            "interface",
            &[
                Stage { kind: gl::VERTEX_SHADER, sources: &[shaders::UI_VERT] },
                Stage { kind: gl::FRAGMENT_SHADER, sources: &[shaders::COMMON, shaders::UI_FRAG] },
            ],
        )?;
        let buffer = Buffer::streaming();
        let attributes = [
            Attribute { size: 2, kind: gl::FLOAT, offset: offset_of!(Vertex, x) },
            Attribute { size: 2, kind: gl::FLOAT, offset: offset_of!(Vertex, u) },
            Attribute { size: 4, kind: gl::UNSIGNED_BYTE, offset: offset_of!(Vertex, color) },
            Attribute { size: 4, kind: gl::FLOAT, offset: offset_of!(Vertex, shape) },
        ];
        let vertex_array = VertexArray::new(&buffer, size_of::<Vertex>(), &attributes, None);
        Ok(Painter { program, buffer, vertex_array, vertices: Vec::new(), fonts: None, width: 0, height: 0 })
    }

    /// The body, heading and display fonts.
    pub fn set_fonts(&mut self, fonts: [Font; 3]) {
        self.fonts = Some(fonts);
    }

    fn font(&self, id: FontId) -> &Font {
        &self.fonts.as_ref().expect("the fonts are set before anything is drawn")[id as usize]
    }

    pub fn begin(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.vertices.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn quad(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, u0: f32, v0: f32, u1: f32, v1: f32, color: Color, shape: Vec4) {
        let vertex = |x, y, u, v| Vertex { x, y, u, v, color, shape };
        let (a, b, c, d) = (vertex(x0, y0, u0, v0), vertex(x1, y0, u1, v0), vertex(x1, y1, u1, v1), vertex(x0, y1, u0, v1));
        self.vertices.extend([a, b, c, a, c, d]);
    }

    /// A quad round a rectangle, its uv the position from the rectangle's center, for the distance shapes.
    fn shape(&mut self, rect: Rect, margin: f32, color: Color, shape: Vec4) {
        let (half_width, half_height) = (rect.width / 2.0, rect.height / 2.0);
        self.quad(
            rect.x - margin,
            rect.y - margin,
            rect.right() + margin,
            rect.bottom() + margin,
            -half_width - margin,
            -half_height - margin,
            half_width + margin,
            half_height + margin,
            color,
            shape,
        );
    }

    pub fn fill(&mut self, rect: Rect, radius: f32, color: Color) {
        let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0);
        self.shape(rect, 1.0, color, vec4(SHAPE_FILL, rect.width / 2.0, rect.height / 2.0, radius));
    }

    pub fn outline(&mut self, rect: Rect, radius: f32, color: Color) {
        let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0);
        self.shape(rect, 1.0, color, vec4(SHAPE_OUTLINE, rect.width / 2.0, rect.height / 2.0, radius));
    }

    pub fn shadow(&mut self, rect: Rect, blur: f32, color: Color) {
        self.shape(rect, blur * 1.5, color, vec4(SHAPE_SHADOW, rect.width / 2.0, rect.height / 2.0, blur));
    }

    pub fn disc(&mut self, x: f32, y: f32, radius: f32, color: Color) {
        self.shape(rect(x - radius, y - radius, radius * 2.0, radius * 2.0), 1.0, color, vec4(SHAPE_DISC, radius, 0.0, 0.0));
    }

    pub fn spinner(&mut self, x: f32, y: f32, radius: f32, thickness: f32, angle: f32, color: Color) {
        self.shape(rect(x - radius, y - radius, radius * 2.0, radius * 2.0), thickness, color, vec4(SHAPE_SPINNER, radius, thickness, angle));
    }

    /// A small triangle pointing left (direction -1) or right (1).
    pub fn arrow(&mut self, x: f32, y: f32, half_width: f32, half_height: f32, direction: f32, color: Color) {
        let bounds = rect(x - half_width, y - half_height, half_width * 2.0, half_height * 2.0);
        self.shape(bounds, 1.0, color, vec4(SHAPE_ARROW, half_width, half_height, direction));
    }

    /// Text whose line starts at the top left point; returns its width.
    pub fn text(&mut self, id: FontId, x: f32, y: f32, text: &str, color: Color) -> f32 {
        let kind = vec4(id as u8 as f32, 0.0, 0.0, 0.0);
        let start = x.round();
        let top = y.round();
        let mut pen = start;
        for character in text.chars() {
            let glyph = *self.font(id).find(character);
            let left = (pen + glyph.left).round();
            let glyph_top = top + glyph.top;
            if glyph.width > 0.0 {
                self.quad(left, glyph_top, left + glyph.width, glyph_top + glyph.height, glyph.u0, glyph.v0, glyph.u1, glyph.v1, color, kind);
            }
            pen += glyph.advance;
        }
        pen - start
    }

    pub fn text_centered(&mut self, id: FontId, rect: Rect, text: &str, color: Color) {
        let width = self.measure(id, text);
        let height = self.font(id).ascent();
        self.text(id, rect.x + (rect.width - width) / 2.0, rect.y + (rect.height - height) / 2.0 - height * 0.04, text, color);
    }

    pub fn measure(&self, id: FontId, text: &str) -> f32 {
        self.font(id).measure(text)
    }

    pub fn line_height(&self, id: FontId) -> f32 {
        self.font(id).line_height()
    }

    /// Draws what has been queued; anything queued later is drawn on top of what was drawn meanwhile.
    pub fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        self.buffer.stream(gfx::bytes_of(&self.vertices));
        gfx::bind_window_framebuffer();
        gfx::viewport(self.width, self.height);
        gfx::disable(gl::DEPTH_TEST);
        gfx::disable(gl::CULL_FACE);
        gfx::enable(gl::BLEND);
        gfx::blend_func_separate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
        self.program.set("uScreen", vec2(self.width as f32, self.height as f32));
        self.program.bind();
        for (unit, font) in (8..).zip(self.fonts.iter().flatten()) {
            font.texture().bind(unit);
        }
        self.vertex_array.bind();
        gfx::draw_triangles(self.vertices.len() as i32);
        gfx::disable(gl::BLEND);
        self.vertices.clear();
    }
}
