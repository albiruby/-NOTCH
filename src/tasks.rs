//! Tasks and daily water intake, stored next to the settings file.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Task {
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub tasks: Vec<Task>,
    pub water: u32,
    pub water_day: String,
    pub water_goal: u32,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            water: 0,
            water_day: today(),
            water_goal: 8,
        }
    }
}

fn path() -> PathBuf {
    crate::settings::Settings::dir().join("tasks.json")
}

pub fn today() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

impl Store {
    pub fn load() -> Self {
        let mut store = std::fs::read_to_string(path())
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(raw.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default();
        store.roll_day();
        store
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::settings::Settings::dir());
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path(), json);
        }
    }

    pub fn roll_day(&mut self) {
        if self.water_day != today() {
            self.water_day = today();
            self.water = 0;
        }
        if self.water_goal == 0 {
            self.water_goal = 8;
        }
    }

    pub fn add(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let text = text.chars().take(72).collect::<String>();
        self.tasks.insert(0, Task { text, done: false });
        if self.tasks.len() > 24 {
            self.tasks.truncate(24);
        }
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(task) = self.tasks.get_mut(index) {
            task.done = !task.done;
        }
    }
}
