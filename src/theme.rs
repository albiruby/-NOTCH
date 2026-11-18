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

    /// Near-clear glass: a light tinted veil, with the sheen and rim doing
    /// the work of separating the surface from the wallpaper.
    pub const GLASS: Recipe = Recipe {
        body_top: 0.16,
        body_bottom: 0.26,
        tile_fill: 0.07,
        sheen: 0.16,
        rim: 0.42,
        shadow: 0.16,
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
    /// Island snap: a little stiffer travel with a small overshoot.
    pub const SNAP_OMEGA: f32 = 24.0;
    pub const SNAP_ZETA: f32 = 0.66;
}

pub mod metric {
    pub const WARNING_THRESHOLD: f32 = 85.0;
    pub const DANGER_THRESHOLD: f32 = 95.0;
    pub const TRACK_HEIGHT: f32 = 3.0;
}

/// Runtime colour theme. Brand marks (the N logo) stay violet; everything
/// that reads as "accent" (sliders, rings, glass tint, rims) follows this.
pub mod accent {
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicU8, Ordering};

    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Theme {
        #[default]
        Violet,
        /// Fresh: mint-teal on blue slate.
        Mint,
        /// Warm: coral to peach on dark plum.
        Coral,
    }

    struct Palette {
        c300: u32,
        c400: u32,
        c500: u32,
        c600: u32,
        c700: u32,
        c800: u32,
        c900: u32,
        base: u32,
        battery: u32,
        /// Glass body hue.
        tint: u32,
        /// Light used for sheen, rims, tiles and hairlines.
        light: u32,
        text1: u32,
        text2: u32,
        muted: u32,
        disabled: u32,
        track: u32,
    }

    const VIOLET: Palette = Palette {
        c300: 0xD4CEFF,
        c400: 0xB9AEFF,
        c500: 0xA194FF,
        c600: 0x8B7CFF,
        c700: 0x7464EB,
        c800: 0x51458F,
        c900: 0x292440,
        base: 0x090A0D,
        battery: super::color::SUCCESS,
        tint: 0x2A2450,
        light: 0xFFFFFF,
        text1: 0xF2F0FF,
        text2: 0xB8B2D8,
        muted: 0x837CA6,
        disabled: 0x4E4870,
        track: 0x2A2645,
    };
    const MINT: Palette = Palette {
        c300: 0xCCFBF1,
        c400: 0x99F6E4,
        c500: 0x5EEAD4,
        c600: 0x2DD4BF,
        c700: 0x14B8A6,
        c800: 0x1F6F6A,
        c900: 0x123038,
        base: 0x06171A,
        battery: super::color::INFO,
        tint: 0x0F4A48,
        light: 0xE6FFFA,
        text1: 0xEFFFFB,
        text2: 0xA9DCD3,
        muted: 0x6FA39B,
        disabled: 0x3D625E,
        track: 0x1C3D3F,
    };
    const CORAL: Palette = Palette {
        c300: 0xFFE4D6,
        c400: 0xFECDA6,
        c500: 0xFDBA74,
        c600: 0xFB7185,
        c700: 0xE65A70,
        c800: 0x8A3A52,
        c900: 0x2E1A2C,
        base: 0x1A0A12,
        battery: super::color::SUCCESS,
        tint: 0x55203A,
        light: 0xFFF1EA,
        text1: 0xFFF4EF,
        text2: 0xEDBFB2,
        muted: 0xB0807B,
        disabled: 0x6E4650,
        track: 0x40222F,
    };

    static CURRENT: AtomicU8 = AtomicU8::new(0);

    pub fn set(theme: Theme) {
        CURRENT.store(theme as u8, Ordering::Relaxed);
    }

    fn p() -> &'static Palette {
        match CURRENT.load(Ordering::Relaxed) {
            1 => &MINT,
            2 => &CORAL,
            _ => &VIOLET,
        }
    }

    pub fn c300() -> u32 { p().c300 }
    pub fn c400() -> u32 { p().c400 }
    pub fn c500() -> u32 { p().c500 }
    pub fn c600() -> u32 { p().c600 }
    pub fn c700() -> u32 { p().c700 }
    pub fn c800() -> u32 { p().c800 }
    pub fn c900() -> u32 { p().c900 }
    /// Darkest glass stop.
    pub fn base() -> u32 { p().base }
    /// Battery ring colour (kept distinct from the accent).
    pub fn battery() -> u32 { p().battery }
    pub fn tint() -> u32 { p().tint }
    pub fn light() -> u32 { p().light }
    pub fn text1() -> u32 { p().text1 }
    pub fn text2() -> u32 { p().text2 }
    pub fn muted() -> u32 { p().muted }
    pub fn disabled() -> u32 { p().disabled }
    pub fn track() -> u32 { p().track }
}
