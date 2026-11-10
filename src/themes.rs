//! The looks of the set: the board, the pieces, the table and the studio light around them.

use std::sync::LazyLock;

use crate::math::{Vec2, Vec3, vec2, vec3};

#[derive(Clone)]
pub struct PieceLook {
    pub albedo: Vec3,
    pub roughness: f32,
    pub metallic: f32,
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
    pub subsurface: f32,
    pub pattern: f32, // 0 none, 1 wood, 2 turned metal, 3 stone
    pub pattern_strength: f32,
    pub trim: Vec3, // a glowing ring round the side; it colors the crown of a king too
    pub trim_strength: f32,
    pub ribs: f32, // fine ribs round the side
}

impl Default for PieceLook {
    fn default() -> PieceLook {
        PieceLook {
            albedo: Vec3::ZERO,
            roughness: 0.3,
            metallic: 0.0,
            clearcoat: 0.0,
            clearcoat_roughness: 0.05,
            subsurface: 0.0,
            pattern: 0.0,
            pattern_strength: 0.0,
            trim: Vec3::ZERO,
            trim_strength: 0.0,
            ribs: 0.0,
        }
    }
}

/// A lit panel of the studio, seen in the reflections.
#[derive(Clone)]
pub struct StudioPanel {
    pub direction: Vec3, // toward the panel
    pub roll: f32,
    pub half_size: Vec2, // as tangents of the angles
    pub corner: f32,
    pub softness: f32,
    pub radiance: Vec3,
    pub falloff: f32,
}

/// A panel that stands upright and fades evenly towards its edges.
fn panel(direction: Vec3, half_size: Vec2, corner: f32, softness: f32, radiance: Vec3) -> StudioPanel {
    StudioPanel { direction, roll: 0.0, half_size, corner, softness, radiance, falloff: 0.3 }
}

#[derive(Clone)]
pub struct Theme {
    pub name_english: &'static str,
    pub name_russian: &'static str,

    // The board.
    pub light_square: Vec3,
    pub dark_square: Vec3,
    pub board_roughness: f32,
    pub board_clearcoat: f32,
    pub board_clearcoat_roughness: f32,
    pub board_pattern: f32, // 0 wood, 1 marble, 2 glass
    pub reflection: f32,
    pub reflection_blur: f32,
    pub vein_brightness: f32,
    pub edge_glow: f32,
    pub frame_color: Vec3,
    pub frame_roughness: f32,
    pub frame_metallic: f32,
    pub frame_clearcoat: f32,
    pub frame_pattern: f32, // 0 wood, 1 marble, 2 metal
    pub inlay_color: Vec3,
    pub inlay_metallic: f32,
    pub inlay_roughness: f32,
    pub inlay_glow: f32,

    pub white: PieceLook,
    pub black: PieceLook,

    pub table_color: Vec3,
    pub table_roughness: f32,
    pub table_pattern: f32, // 0 felt, 1 wood, 2 glowing grid, 3 stone
    pub table_sheen: f32,
    pub grid_glow: f32,

    // The light.
    pub key_direction: Vec3,
    pub key_radiance: Vec3,
    pub key_softness: f32, // tangent of the light's angular radius
    pub ceiling: Vec3,
    pub horizon: Vec3,
    pub floor: Vec3,
    pub panels: Vec<StudioPanel>, // eight at most
    pub diffuse_light: f32,
    pub reflected_light: f32,
    pub background: f32,
    pub fog: Vec3,
    pub exposure: f32,
    pub bloom: f32,
    pub vignette: f32,

    pub accent: Vec3,      // highlights and the interface
    pub accent_warm: Vec3, // the last move
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            name_english: "",
            name_russian: "",
            light_square: Vec3::ZERO,
            dark_square: Vec3::ZERO,
            board_roughness: 0.3,
            board_clearcoat: 0.0,
            board_clearcoat_roughness: 0.05,
            board_pattern: 0.0,
            reflection: 0.5,
            reflection_blur: 1.5,
            vein_brightness: 0.5,
            edge_glow: 0.0,
            frame_color: Vec3::ZERO,
            frame_roughness: 0.35,
            frame_metallic: 0.0,
            frame_clearcoat: 0.0,
            frame_pattern: 0.0,
            inlay_color: Vec3::ZERO,
            inlay_metallic: 1.0,
            inlay_roughness: 0.3,
            inlay_glow: 0.0,
            white: PieceLook::default(),
            black: PieceLook::default(),
            table_color: Vec3::ZERO,
            table_roughness: 0.9,
            table_pattern: 0.0,
            table_sheen: 0.0,
            grid_glow: 0.0,
            key_direction: Vec3::Y,
            key_radiance: Vec3::ZERO,
            key_softness: 0.04,
            ceiling: Vec3::ZERO,
            horizon: Vec3::ZERO,
            floor: Vec3::ZERO,
            panels: Vec::new(),
            diffuse_light: 1.0,
            reflected_light: 1.0,
            background: 0.6,
            fog: Vec3::ZERO,
            exposure: 1.0,
            bloom: 0.05,
            vignette: 0.35,
            accent: Vec3::ZERO,
            accent_warm: Vec3::ZERO,
        }
    }
}

pub const THEME_COUNT: usize = 3;

pub fn themes() -> &'static [Theme; THEME_COUNT] {
    static THEMES: LazyLock<[Theme; THEME_COUNT]> = LazyLock::new(|| [walnut(), marble(), neon()]);
    &THEMES
}

fn walnut() -> Theme {
    Theme {
        name_english: "Walnut",
        name_russian: "Орех",
        light_square: vec3(0.62, 0.43, 0.24),
        dark_square: vec3(0.17, 0.085, 0.04),
        board_roughness: 0.32,
        board_clearcoat: 0.8,
        board_clearcoat_roughness: 0.07,
        board_pattern: 0.0,
        reflection: 0.85,
        reflection_blur: 1.6,
        frame_color: vec3(0.09, 0.045, 0.022),
        frame_roughness: 0.38,
        frame_clearcoat: 0.9,
        frame_pattern: 0.0,
        inlay_color: vec3(0.95, 0.7, 0.36),
        inlay_metallic: 1.0,
        inlay_roughness: 0.28,
        white: PieceLook {
            albedo: vec3(0.78, 0.58, 0.34),
            roughness: 0.36,
            clearcoat: 0.85,
            clearcoat_roughness: 0.06,
            subsurface: 0.2,
            pattern: 1.0,
            pattern_strength: 0.8,
            ..Default::default()
        },
        black: PieceLook {
            albedo: vec3(0.075, 0.03, 0.018),
            roughness: 0.32,
            clearcoat: 1.0,
            clearcoat_roughness: 0.045,
            pattern: 1.0,
            pattern_strength: 0.65,
            ..Default::default()
        },
        table_color: vec3(0.035, 0.075, 0.055),
        table_roughness: 0.95,
        table_pattern: 0.0,
        table_sheen: 0.35,
        key_direction: vec3(-0.58, 0.7, 0.42),
        key_radiance: vec3(4.6, 4.1, 3.5),
        key_softness: 0.045,
        ceiling: vec3(0.035, 0.033, 0.034),
        horizon: vec3(0.11, 0.085, 0.065),
        floor: vec3(0.02, 0.016, 0.012),
        diffuse_light: 0.62,
        reflected_light: 1.0,
        background: 0.55,
        fog: vec3(0.035, 0.027, 0.022),
        exposure: 1.1,
        bloom: 0.045,
        vignette: 0.42,
        accent: vec3(0.35, 0.75, 1.0),
        accent_warm: vec3(1.0, 0.78, 0.32),
        panels: vec![
            panel(vec3(-0.58, 0.7, 0.42), vec2(0.42, 0.3), 0.08, 0.06, vec3(7.5, 6.6, 5.6)),
            panel(vec3(0.85, 0.3, 0.3), vec2(0.55, 0.35), 0.1, 0.15, vec3(0.7, 0.78, 0.9)),
            panel(vec3(0.15, 0.42, -0.9), vec2(1.3, 0.07), 0.03, 0.03, vec3(5.5, 5.0, 4.4)),
            panel(vec3(0.0, 1.0, 0.05), vec2(0.7, 0.7), 0.3, 0.3, vec3(0.6, 0.58, 0.55)),
            panel(vec3(-0.9, 0.25, -0.4), vec2(0.06, 0.6), 0.03, 0.03, vec3(2.6, 2.2, 1.8)),
        ],
        ..Default::default()
    }
}

fn marble() -> Theme {
    Theme {
        name_english: "Marble",
        name_russian: "Мрамор",
        light_square: vec3(0.78, 0.77, 0.74),
        dark_square: vec3(0.035, 0.06, 0.05),
        board_roughness: 0.12,
        board_clearcoat: 0.3,
        board_clearcoat_roughness: 0.03,
        board_pattern: 1.0,
        reflection: 1.0,
        reflection_blur: 0.6,
        vein_brightness: 0.35,
        frame_color: vec3(0.02, 0.02, 0.022),
        frame_roughness: 0.14,
        frame_clearcoat: 0.5,
        frame_pattern: 1.0,
        inlay_color: vec3(0.98, 0.8, 0.45),
        inlay_metallic: 1.0,
        inlay_roughness: 0.2,
        white: PieceLook {
            albedo: vec3(0.8, 0.79, 0.76),
            roughness: 0.18,
            clearcoat: 0.8,
            clearcoat_roughness: 0.04,
            subsurface: 0.35,
            pattern: 3.0,
            pattern_strength: 0.25,
            ..Default::default()
        },
        black: PieceLook {
            albedo: vec3(0.02, 0.03, 0.028),
            roughness: 0.08,
            clearcoat: 1.0,
            clearcoat_roughness: 0.02,
            pattern: 3.0,
            pattern_strength: 0.35,
            ..Default::default()
        },
        table_color: vec3(0.022, 0.022, 0.026),
        table_roughness: 0.3,
        table_pattern: 3.0,
        key_direction: vec3(-0.42, 0.85, 0.32),
        key_radiance: vec3(3.0, 3.05, 3.2),
        key_softness: 0.05,
        ceiling: vec3(0.05, 0.055, 0.065),
        horizon: vec3(0.09, 0.095, 0.11),
        floor: vec3(0.018, 0.018, 0.02),
        diffuse_light: 1.0,
        reflected_light: 1.0,
        background: 0.5,
        fog: vec3(0.03, 0.032, 0.038),
        exposure: 1.0,
        bloom: 0.04,
        vignette: 0.4,
        accent: vec3(0.4, 0.85, 0.75),
        accent_warm: vec3(0.95, 0.75, 0.35),
        panels: vec![
            panel(vec3(-0.42, 0.85, 0.32), vec2(0.5, 0.5), 0.25, 0.12, vec3(6.0, 6.1, 6.4)),
            panel(vec3(0.9, 0.2, 0.25), vec2(0.5, 0.3), 0.1, 0.12, vec3(0.8, 0.85, 1.0)),
            panel(vec3(-0.1, 0.35, -0.92), vec2(1.4, 0.05), 0.02, 0.02, vec3(6.0, 6.2, 6.6)),
            panel(vec3(0.0, 1.0, 0.0), vec2(0.8, 0.8), 0.4, 0.35, vec3(0.9, 0.92, 1.0)),
            panel(vec3(0.6, 0.3, -0.75), vec2(0.05, 0.7), 0.02, 0.02, vec3(2.5, 2.6, 3.0)),
        ],
        ..Default::default()
    }
}

fn neon() -> Theme {
    Theme {
        name_english: "Neon",
        name_russian: "Неон",
        light_square: vec3(0.11, 0.17, 0.24),
        dark_square: vec3(0.006, 0.008, 0.014),
        board_roughness: 0.08,
        board_clearcoat: 0.6,
        board_clearcoat_roughness: 0.02,
        board_pattern: 2.0,
        reflection: 1.0,
        reflection_blur: 0.4,
        edge_glow: 0.6,
        frame_color: vec3(0.09, 0.1, 0.12),
        frame_roughness: 0.3,
        frame_metallic: 1.0,
        frame_pattern: 2.0,
        inlay_color: vec3(0.2, 0.75, 1.0),
        inlay_metallic: 0.0,
        inlay_roughness: 0.3,
        inlay_glow: 3.0,
        white: PieceLook {
            albedo: vec3(0.82, 0.86, 0.9),
            roughness: 0.22,
            clearcoat: 1.0,
            clearcoat_roughness: 0.03,
            subsurface: 0.5,
            trim: vec3(0.2, 0.8, 1.0),
            trim_strength: 5.0,
            ribs: 1.0,
            ..Default::default()
        },
        black: PieceLook {
            albedo: vec3(0.36, 0.36, 0.42),
            roughness: 0.26,
            metallic: 1.0,
            pattern: 2.0,
            pattern_strength: 1.0,
            trim: vec3(1.0, 0.3, 0.75),
            trim_strength: 5.0,
            ribs: 1.0,
            ..Default::default()
        },
        table_color: vec3(0.004, 0.006, 0.01),
        table_roughness: 0.5,
        table_pattern: 2.0,
        grid_glow: 0.55,
        key_direction: vec3(-0.35, 0.8, 0.48),
        key_radiance: vec3(1.8, 2.1, 2.6),
        key_softness: 0.03,
        ceiling: vec3(0.01, 0.012, 0.025),
        horizon: vec3(0.02, 0.035, 0.07),
        floor: vec3(0.004, 0.005, 0.008),
        diffuse_light: 1.0,
        reflected_light: 1.2,
        background: 0.7,
        fog: vec3(0.006, 0.01, 0.02),
        exposure: 1.1,
        bloom: 0.07,
        vignette: 0.5,
        accent: vec3(0.2, 0.8, 1.0),
        accent_warm: vec3(1.0, 0.35, 0.75),
        panels: vec![
            panel(vec3(-0.35, 0.8, 0.48), vec2(0.35, 0.25), 0.05, 0.05, vec3(3.5, 4.2, 5.0)),
            panel(vec3(0.95, 0.12, 0.2), vec2(1.0, 0.03), 0.02, 0.02, vec3(8.0, 1.5, 5.0)),
            panel(vec3(-0.95, 0.12, -0.2), vec2(1.0, 0.03), 0.02, 0.02, vec3(1.0, 5.0, 8.0)),
            panel(vec3(0.1, 0.3, -0.95), vec2(1.4, 0.025), 0.01, 0.015, vec3(8.0, 4.0, 1.0)),
            panel(vec3(0.0, 1.0, 0.0), vec2(0.6, 0.6), 0.3, 0.3, vec3(0.25, 0.3, 0.45)),
            panel(vec3(0.3, 0.25, 0.95), vec2(0.9, 0.025), 0.01, 0.015, vec3(1.0, 6.0, 7.0)),
        ],
        ..Default::default()
    }
}
