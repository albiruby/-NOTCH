//! User settings, stored as JSON under %APPDATA%\Notch.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::state::{Layout, SurfaceMode};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: SurfaceMode,
    pub layout: Layout,
    pub opacity: f32,
    pub compact_width: f32,
    pub compact_height: f32,
    pub expanded_width: f32,
    pub expanded_height: f32,
    pub hide_on_fullscreen: bool,
    pub show_clock: bool,
    /// Dynamic-island pill centre as a fraction of the monitor (x, y).
    /// `None` keeps the default top-centre spot.
    pub island_anchor: Option<[f32; 2]>,
    /// Accent colour theme: `violet` (default), `mint`, or `coral`.
    pub theme: crate::theme::accent::Theme,
    /// Show the idle mascot (a pair of eyes) when nothing is happening.
    pub mascot: bool,
    /// Minimal notch: the closed pill becomes a thin line.
    pub minimal: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: SurfaceMode::Island,
            layout: Layout::Hub,
            opacity: 1.0,
            compact_width: 200.0,
            compact_height: 34.0,
            expanded_width: 460.0,
            expanded_height: 300.0,
            hide_on_fullscreen: true,
            show_clock: true,
            island_anchor: None,
            theme: Default::default(),
            mascot: true,
            minimal: false,
        }
    }
}

impl Settings {
    pub fn dir() -> PathBuf {
        match std::env::var_os("APPDATA") {
            Some(base) => PathBuf::from(base).join("Notch"),
            None => PathBuf::from("."),
        }
    }

    pub fn path() -> PathBuf {
        Self::dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let Ok(raw) = std::fs::read_to_string(&path) else {
            let settings = Self::default();
            settings.save();
            return settings;
        };
        match serde_json::from_str(raw.trim_start_matches('\u{feff}')) {
            Ok(settings) => settings,
            Err(err) => {
                eprintln!("settings parse failed ({err}), using defaults");
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let dir = Self::dir();
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("settings dir failed: {err}");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(err) = std::fs::write(Self::path(), json) {
                    eprintln!("settings write failed: {err}");
                }
            }
            Err(err) => eprintln!("settings encode failed: {err}"),
        }
    }
}
