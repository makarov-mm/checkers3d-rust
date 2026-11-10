//! The system's fonts drawn into textures, and text measured and laid out from them.

use std::collections::HashMap;
use std::rc::Rc;

use crate::Result;
use crate::gfx::Texture;

/// The weights of the faces the game asks for.
pub const WEIGHT_NORMAL: u16 = 400;
pub const WEIGHT_SEMIBOLD: u16 = 600;

// The faces the game was drawn for, each followed by the ones that stand in for it on other systems.
const SANS: &[&str] = &["Segoe UI", "Noto Sans", "Inter", "Roboto", "Liberation Sans", "Helvetica Neue", "Arial", "DejaVu Sans"];
const SYMBOLS: &[&str] = &["Segoe UI Symbol", "Noto Sans Symbols2", "Noto Sans Symbols", "DejaVu Sans", "Apple Symbols", "Arial Unicode MS"];
const SERIF: &[&str] = &["Georgia", "Noto Serif", "DejaVu Serif", "Liberation Serif", "Times New Roman"];

/// The fonts installed on the system, and the faces already read from them.
pub struct FontLibrary {
    database: fontdb::Database,
    faces: HashMap<fontdb::ID, Rc<fontdue::Font>>,
}

impl FontLibrary {
    pub fn new() -> FontLibrary {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        FontLibrary { database, faces: HashMap::new() }
    }

    /// The first of the families that the system has, in the weight nearest to the one asked for. With a generic
    /// family to fall back on, a system that has none of them gives whatever font it has.
    fn face(&mut self, families: &[&str], generic: Option<fontdb::Family>, weight: u16) -> Option<Rc<fontdue::Font>> {
        let families: Vec<fontdb::Family> = families.iter().map(|name| fontdb::Family::Name(name)).chain(generic).collect();
        let query = fontdb::Query { families: &families, weight: fontdb::Weight(weight), ..Default::default() };
        let any = || generic.and_then(|_| self.database.faces().next().map(|face| face.id));
        let id = self.database.query(&query).or_else(any)?;
        if let Some(face) = self.faces.get(&id) {
            return Some(face.clone());
        }
        let face = self.database.with_face_data(id, |data, index| {
            let settings = fontdue::FontSettings { collection_index: index, scale: 64.0, ..Default::default() };
            fontdue::Font::from_bytes(data, settings).ok()
        })??;
        let face = Rc::new(face);
        self.faces.insert(id, face.clone());
        Some(face)
    }

    /// The faces to draw the interface with: the letters' face first, then those with the symbols it lacks.
    fn interface_faces(&mut self, weight: u16) -> Result<Vec<Rc<fontdue::Font>>> {
        let main = self.face(SANS, Some(fontdb::Family::SansSerif), weight).ok_or("The system has no font to draw the text with.")?;
        let mut faces = vec![main];
        for family in SYMBOLS {
            if let Some(face) = self.face(&[family], None, weight) {
                if !faces.iter().any(|known| Rc::ptr_eq(known, &face)) {
                    faces.push(face);
                }
            }
        }
        Ok(faces)
    }
}

/// The ascent and the descent of a face at a size, in whole pixels as the lines are laid out.
fn vertical_metrics(face: &fontdue::Font, px: f32) -> (i32, i32, i32) {
    match face.horizontal_line_metrics(px) {
        Some(metrics) => (metrics.ascent.round() as i32, (-metrics.descent).round() as i32, metrics.line_gap.round() as i32),
        None => ((px * 0.8).round() as i32, (px * 0.2).round() as i32, 0),
    }
}

/// A glyph's coverage, one byte a pixel, with its place from the pen and the baseline.
struct Raster {
    left: i32,
    top: i32, // below the baseline when positive
    width: i32,
    height: i32,
    coverage: Vec<u8>,
}

fn rasterize(face: &fontdue::Font, character: char, px: f32) -> Raster {
    let (metrics, coverage) = face.rasterize(character, px);
    Raster { left: metrics.xmin, top: -(metrics.ymin + metrics.height as i32), width: metrics.width as i32, height: metrics.height as i32, coverage }
}

#[derive(Clone, Copy, Default)]
pub struct Glyph {
    // In the atlas.
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    // Of the quad, in pixels.
    pub width: f32,
    pub height: f32,
    // From the pen and the line's top to the quad.
    pub left: f32,
    pub top: f32,
    pub advance: f32,
}

/// A font of a given pixel height, with the Latin and Cyrillic letters and a few symbols.
pub struct Font {
    glyphs: HashMap<char, Glyph>,
    fallback: Glyph,
    line_height: f32,
    ascent: f32,
    texture: Texture,
}

fn characters() -> impl Iterator<Item = char> {
    const SYMBOLS: [char; 22] = ['Ё', 'ё', '—', '–', '…', '•', '·', '←', '→', '↑', '↓', '×', '½', '«', '»', '№', '↶', '↷', '▲', '▼', '◀', '▶'];
    (' '..='~').chain('А'..='я').chain(SYMBOLS)
}

impl Font {
    pub fn new(library: &mut FontLibrary, pixel_height: f32, weight: u16) -> Result<Font> {
        struct Item {
            character: char,
            face: usize,
            advance: i32,
            x: i32,
            y: i32,
        }

        let height = (pixel_height.round() as i32).max(6);
        let px = height as f32;
        let faces = library.interface_faces(weight)?;
        let (main_ascent, main_descent, line_gap) = vertical_metrics(&faces[0], px);

        // The letters come from the first face; a symbol it lacks from the first of the others that has it.
        let mut items = Vec::new();
        for character in characters() {
            let face = if character.is_ascii() { Some(0) } else { faces.iter().position(|face| face.has_glyph(character)) };
            if let Some(face) = face {
                let advance = faces[face].metrics(character, px).advance_width.round() as i32;
                items.push(Item { character, face, advance, x: 0, y: 0 });
            }
        }

        // Cells in rows, with room for overhanging strokes.
        let pad = (height / 6).max(2);
        let (mut ascent, mut descent) = (main_ascent, main_descent);
        for item in &items {
            let (face_ascent, face_descent, _) = vertical_metrics(&faces[item.face], px);
            ascent = ascent.max(face_ascent);
            descent = descent.max(face_descent);
        }
        let cell_height = ascent + descent + 2 * pad;
        let mut atlas_width = 256;
        let atlas_height = loop {
            let (mut x, mut y) = (0, 0);
            for item in &mut items {
                let cell_width = item.advance + 2 * pad;
                if x + cell_width > atlas_width {
                    x = 0;
                    y += cell_height;
                }
                (item.x, item.y) = (x, y);
                x += cell_width;
            }
            if y + cell_height <= atlas_width {
                break y + cell_height;
            }
            atlas_width *= 2;
        };

        let mut coverage = vec![0u8; (atlas_width * atlas_height) as usize];
        for item in &items {
            let raster = rasterize(&faces[item.face], item.character, px);
            let cell_width = item.advance + 2 * pad;
            // A stroke that reaches out of its cell is cut off there, not drawn into the neighbour's.
            for row in 0..raster.height {
                let y = pad + ascent + raster.top + row;
                if y < 0 || y >= cell_height {
                    continue;
                }
                for column in 0..raster.width {
                    let x = pad + raster.left + column;
                    if x >= 0 && x < cell_width {
                        coverage[((item.y + y) * atlas_width + item.x + x) as usize] = raster.coverage[(row * raster.width + column) as usize];
                    }
                }
            }
        }

        let texture = Texture::new(gl::TEXTURE_2D);
        texture.storage(1, gl::R8, atlas_width, atlas_height);
        texture.upload_r8(atlas_width, atlas_height, &coverage);
        texture.set_filter(gl::LINEAR, gl::LINEAR, gl::CLAMP_TO_EDGE);

        let (w, h) = (atlas_width as f32, atlas_height as f32);
        let glyphs: HashMap<char, Glyph> = items
            .iter()
            .map(|item| {
                let cell_width = item.advance + 2 * pad;
                let glyph = Glyph {
                    u0: item.x as f32 / w,
                    v0: item.y as f32 / h,
                    u1: (item.x + cell_width) as f32 / w,
                    v1: (item.y + cell_height) as f32 / h,
                    width: cell_width as f32,
                    height: cell_height as f32,
                    left: -pad as f32,
                    top: (main_ascent - ascent - pad) as f32,
                    advance: item.advance as f32,
                };
                (item.character, glyph)
            })
            .collect();
        let fallback = glyphs.get(&'?').copied().unwrap_or_default();

        Ok(Font { glyphs, fallback, line_height: (main_ascent + main_descent + line_gap) as f32, ascent: main_ascent as f32, texture })
    }

    pub fn find(&self, character: char) -> &Glyph {
        self.glyphs.get(&character).unwrap_or(&self.fallback)
    }

    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    pub fn measure(&self, text: &str) -> f32 {
        text.chars().map(|character| self.find(character).advance).sum()
    }

    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}

/// The coordinates inlaid in the board's frame, as coverage over the square from -extent to extent in x and z.
pub fn render_board_labels(library: &mut FontLibrary, size: i32, extent: f32, inner: f32) -> Vec<u8> {
    let mut coverage = vec![0u8; (size * size) as usize];
    let Some(face) = library.face(SERIF, Some(fontdb::Family::Serif), WEIGHT_SEMIBOLD) else {
        return coverage; // a frame without coordinates
    };
    let pixels_per_unit = size as f32 / (2.0 * extent);
    let px = (0.3 * pixels_per_unit).floor();
    let middle = (inner + extent) / 2.0 - 0.02;
    let to_pixel = |world: f32| ((world + extent) * pixels_per_unit).round() as i32;
    let (_, descent, _) = vertical_metrics(&face, px);
    let half_cap = (px - descent as f32) * 0.5 / pixels_per_unit;

    let mut draw = |character: char, x: f32, z: f32, for_white: bool| {
        let raster = rasterize(&face, character, px);
        let advance = face.metrics(character, px).advance_width.round() as i32;
        // Centered on the point: the baseline below it for White, above it for Black, who sees the character
        // turned half round about the point on its baseline.
        let (origin_x, origin_y) = (to_pixel(x), to_pixel(z + if for_white { half_cap } else { -half_cap }));
        for row in 0..raster.height {
            for column in 0..raster.width {
                let (dx, dy) = (raster.left + column - advance / 2, raster.top + row);
                let (tx, ty) = if for_white { (origin_x + dx, origin_y + dy) } else { (origin_x - 1 - dx, origin_y - 1 - dy) };
                if tx >= 0 && tx < size && ty >= 0 && ty < size {
                    let target = &mut coverage[(ty * size + tx) as usize];
                    *target = (*target).max(raster.coverage[(row * raster.width + column) as usize]);
                }
            }
        }
    };

    for i in 0..8u8 {
        let offset = f32::from(i) - 3.5;
        draw(char::from(b'a' + i), offset, middle, true);
        draw(char::from(b'a' + i), offset, -middle, false);
        draw(char::from(b'1' + i), -middle, -offset, true);
        draw(char::from(b'1' + i), middle, -offset, false);
    }
    coverage
}
