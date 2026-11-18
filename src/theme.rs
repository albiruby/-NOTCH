//! Design tokens for NOTCH Design System v1.0.
//!
//! Single source of truth for the values the renderer consumes. The matching
//! JSON files live in `design-system/tokens/`; keep both in sync.

#![allow(dead_code)]

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub const fn rgb(hex: u32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: alpha,
    }
}

pub mod color {
    pub const GRAPHITE_950: u32 = 0x090A0D;
    pub const GRAPHITE_900: u32 = 0x101216;
    pub const GRAPHITE_850: u32 = 0x14171D;
    pub const GRAPHITE_800: u32 = 0x171A20;
    pub const GRAPHITE_750: u32 = 0x1C2028;
    pub const GRAPHITE_700: u32 = 0x20242D;
    pub const GRAPHITE_600: u32 = 0x2B303A;
    pub const GRAPHITE_500: u32 = 0x414856;
    pub const GRAPHITE_400: u32 = 0x707989;
    pub const GRAPHITE_300: u32 = 0xA0A6B2;
    pub const GRAPHITE_200: u32 = 0xC8CCD5;
    pub const GRAPHITE_100: u32 = 0xE8EAF0;

    pub const VIOLET_950: u32 = 0x171329;
    pub const VIOLET_900: u32 = 0x292440;
    pub const VIOLET_800: u32 = 0x51458F;
    pub const VIOLET_700: u32 = 0x7464EB;
    pub const VIOLET_600: u32 = 0x8B7CFF;
    pub const VIOLET_500: u32 = 0xA194FF;
    pub const VIOLET_400: u32 = 0xB9AEFF;
    pub const VIOLET_300: u32 = 0xD4CEFF;

    pub const SUCCESS: u32 = 0x4ADE80;
    pub const WARNING: u32 = 0xFBBF24;
    pub const DANGER: u32 = 0xFB7185;
    pub const INFO: u32 = 0x60A5FA;

    // Semantic aliases. Components use these, never the primitives.
    pub const BG_CANVAS: u32 = GRAPHITE_950;
    pub const BG_BASE: u32 = GRAPHITE_900;
    pub const BG_SURFACE: u32 = GRAPHITE_800;
    pub const BG_INTERACTIVE: u32 = GRAPHITE_750;
    pub const BG_ELEVATED: u32 = GRAPHITE_700;

    pub const TEXT_PRIMARY: u32 = GRAPHITE_100;
    pub const TEXT_SECONDARY: u32 = GRAPHITE_300;
    pub const TEXT_MUTED: u32 = GRAPHITE_400;
    pub const TEXT_DISABLED: u32 = GRAPHITE_500;

    pub const BORDER_SUBTLE: u32 = GRAPHITE_600;
    pub const BORDER_STRONG: u32 = GRAPHITE_500;

    pub const ACCENT_PRIMARY: u32 = VIOLET_600;
    pub const ACCENT_HOVER: u32 = VIOLET_500;
    pub const ACCENT_ACTIVE: u32 = VIOLET_700;
}

pub mod glass {
    /// Strong glass: a violet-tinted layer that stays readable without blur.
    pub const STRONG_TOP: f32 = 0.90;
    pub const STRONG_MID: f32 = 0.86;
    pub const STRONG_BOTTOM: f32 = 0.84;
    pub const OPACITY_SUBTLE: f32 = 0.68;
    pub const OPACITY_STANDARD: f32 = 0.80;
    pub const OPACITY_SOLID: f32 = 0.90;
    pub const BORDER_OPACITY: f32 = 0.30;
    pub const HIGHLIGHT_OPACITY: f32 = 0.10;

    /// Glassmorphism recipe used by `renderer::draw_glass`.
    pub struct Recipe {
        pub body_top: f32,
        pub body_bottom: f32,
        pub tile_fill: f32,
        pub sheen: f32,
        pub rim: f32,
        pub shadow: f32,
    }

    pub const GLASS: Recipe = Recipe {
        body_top: 0.58,
        body_bottom: 0.70,
        tile_fill: 0.06,
        sheen: 0.12,
        rim: 0.30,
        shadow: 0.22,
    };
}

pub mod space {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S5: f32 = 20.0;
    pub const S6: f32 = 24.0;
    pub const S8: f32 = 32.0;
}

pub mod radius {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 10.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 22.0;
    pub const XXL: f32 = 28.0;
}

pub mod typography {
    pub const FONT_UI: &str = "Segoe UI Variable";
    pub const FONT_MONO: &str = "Cascadia Code";
    pub const DISPLAY: f32 = 32.0;
    pub const BODY_SM: f32 = 13.0;
    pub const LABEL: f32 = 12.0;
    pub const CAPTION: f32 = 11.0;
    pub const MICRO: f32 = 10.0;
}

pub mod motion {
    pub const INSTANT_MS: u16 = 80;
    pub const FAST_MS: u16 = 120;
    pub const NORMAL_MS: u16 = 180;
    pub const SMOOTH_MS: u16 = 220;
    pub const SLOW_MS: u16 = 300;
    /// Spring constant for surface morphing, tuned to land near motion.smooth.
    pub const SPRING_OMEGA: f32 = 30.0;
}

pub mod metric {
    pub const WARNING_THRESHOLD: f32 = 85.0;
    pub const DANGER_THRESHOLD: f32 = 95.0;
    pub const TRACK_HEIGHT: f32 = 3.0;
}
