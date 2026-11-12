//! File shelf: paths dropped onto the notch, persisted between runs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub paths: Vec<String>,
}

fn path() -> PathBuf {
    crate::settings::Settings::dir().join("shelf.json")
}

impl Store {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(raw.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::settings::Settings::dir());
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path(), json);
        }
    }

    pub fn add(&mut self, paths: Vec<String>) {
        if paths.is_empty() {
            return;
        }
        let mut changed = false;
        for path in paths {
            if path.is_empty() || self.paths.contains(&path) {
                continue;
            }
            self.paths.insert(0, path);
            changed = true;
        }
        if self.paths.len() > 8 {
            self.paths.truncate(8);
            changed = true;
        }
        if changed {
            self.save();
        }
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.save();
    }
}
