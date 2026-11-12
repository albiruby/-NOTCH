//! Installed application shortcuts for the apps launcher.
//!
//! Reads the taskbar-pinned shortcuts first (the set a user actually uses),
//! then falls back to the Start Menu. Names come from the shortcut filename;
//! launching goes through the shell so the real application opens.

use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
}

pub fn load() -> Vec<AppEntry> {
    let mut entries = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let pinned = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Internet Explorer")
            .join("Quick Launch")
            .join("User Pinned")
            .join("TaskBar");
        scan(&pinned, 1, &mut entries);
    }
    if entries.is_empty() {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let start = PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs");
            scan(&start, 2, &mut entries);
        }
        if let Some(programdata) = std::env::var_os("PROGRAMDATA") {
            let start = PathBuf::from(programdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs");
            scan(&start, 2, &mut entries);
        }
    }
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    entries.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    entries.truncate(48);
    entries
}

fn scan(dir: &Path, depth: u32, out: &mut Vec<AppEntry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                scan(&path, depth - 1, out);
            }
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(ext.to_ascii_lowercase().as_str(), "lnk" | "url") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = stem
            .trim_end_matches(" - Shortcut")
            .trim_end_matches(" - Shortcut.lnk")
            .trim()
            .to_string();
        if name.is_empty() || name.to_lowercase().contains("uninstall") {
            continue;
        }
        out.push(AppEntry {
            name,
            path: path.to_string_lossy().to_string(),
        });
    }
}
