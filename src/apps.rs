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

/// Loads a shortcut's icon as premultiplied BGRA pixels (top-down).
/// Returns (side, pixels) where the image is `side` x `side`. None if the
/// icon can't be read. Used by the renderer to build a D2D bitmap once.
pub fn icon_bgra(path: &str) -> Option<(u32, Vec<u8>)> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon: HICON = info.hIcon;
        let result = icon_to_bgra(icon);
        let _ = DestroyIcon(icon);
        return result;
    }

    /// Inner: HICON -> pixels via its colour bitmap.
    unsafe fn icon_to_bgra(icon: HICON) -> Option<(u32, Vec<u8>)> {
        let mut ii = ICONINFO::default();
        GetIconInfo(icon, &mut ii).ok()?;
        let color: HBITMAP = ii.hbmColor;
        if !ii.hbmMask.is_invalid() {
            let _ = DeleteObject(ii.hbmMask.into());
        }
        if color.is_invalid() {
            return None;
        }
        let mut bmp = BITMAP::default();
        let got = GetObjectW(
            color.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut BITMAP as *mut core::ffi::c_void),
        );
        if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
            let _ = DeleteObject(color.into());
            return None;
        }
        let w = bmp.bmWidth as u32;
        let h = bmp.bmHeight as u32;
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bmp.bmWidth,
                // Negative height: top-down rows.
                biHeight: -bmp.bmHeight,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(None);
        let scans = GetDIBits(
            dc,
            color,
            0,
            h,
            Some(pixels.as_mut_ptr() as *mut core::ffi::c_void),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        let _ = DeleteObject(color.into());
        if scans == 0 {
            return None;
        }
        // GetDIBits gives straight-alpha BGRA; D2D premultiplied needs b,g,r
        // scaled by a. Some icons report zero alpha everywhere (no alpha
        // channel) — treat those as fully opaque.
        let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
        for px in pixels.chunks_exact_mut(4) {
            let a = if has_alpha { px[3] as u32 } else { 255 };
            px[0] = (px[0] as u32 * a / 255) as u8;
            px[1] = (px[1] as u32 * a / 255) as u8;
            px[2] = (px[2] as u32 * a / 255) as u8;
            px[3] = a as u8;
        }
        (w == h).then_some((w, pixels))
    }
}
