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

/// The icon source of a .lnk: its explicit icon file if set, else the target
/// path. None for other files or shortcuts without a file target (e.g. Store
/// apps), which fall back to the shell icon.
fn link_target(path: &str) -> Option<String> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    if !path.to_ascii_lowercase().ends_with(".lnk") {
        return None;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.cast::<IPersistFile>()
            .ok()?
            .Load(PCWSTR(wide.as_ptr()), STGM_READ)
            .ok()?;
        let mut buf = [0u16; 520];
        let mut index = 0i32;
        let icon = link
            .GetIconLocation(&mut buf, &mut index)
            .ok()
            .map(|_| String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]))
            .filter(|s| !s.is_empty() && !s.to_ascii_lowercase().ends_with(".lnk"));
        // An explicit icon file only helps when it is the main icon; with an
        // index we would pick the wrong one, so prefer the target then.
        if let Some(icon) = icon.filter(|_| index == 0) {
            let expanded = std::env::var("SystemRoot")
                .map(|root| icon.replace("%SystemRoot%", &root).replace("%windir%", &root))
                .unwrap_or(icon);
            if std::path::Path::new(&expanded).exists() {
                return Some(expanded);
            }
        }
        let mut buf = [0u16; 520];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let target = String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]);
        (!target.is_empty() && std::path::Path::new(&target).exists()).then_some(target)
    }
}

/// A `size`-px icon image from the shell (IShellItemImageFactory), as
/// premultiplied BGRA. Skips results that are only an upscaled small icon.
fn shell_image(path: &str, size: i32) -> Option<(u32, Vec<u8>)> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    };
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(
            PCWSTR(wide.as_ptr()),
            None::<&windows::Win32::System::Com::IBindCtx>,
        )
        .ok()?;
        let hbmp = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)
            .ok()?;
        let mut bmp = BITMAP::default();
        let got = GetObjectW(
            hbmp.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut BITMAP as *mut core::ffi::c_void),
        );
        let (w, h) = (bmp.bmWidth, bmp.bmHeight.abs());
        if got == 0 || w <= 0 || w != h {
            let _ = DeleteObject(hbmp.into());
            return None;
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(None);
        let scans = GetDIBits(dc, hbmp, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        ReleaseDC(None, dc);
        let _ = DeleteObject(hbmp.into());
        if scans == 0 || pixels.chunks_exact(4).all(|p| p[3] == 0) {
            return None;
        }
        // The shell hands back premultiplied alpha already.
        Some((w as u32, pixels))
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
    // Sharp icons first: ask the shell for a 128 px image of the shortcut's
    // target (no shortcut arrow), then of the shortcut itself.
    let target = link_target(path);
    for source in target.iter().map(String::as_str).chain(std::iter::once(path)) {
        if let Some(found) = shell_image(source, 128) {
            return Some(found);
        }
    }
    unsafe {
        // SHGetFileInfo on a .lnk paints the shortcut arrow over the icon.
        // Resolve the shortcut to its target and ask for that icon instead.
        if let Some(target) = link_target(path) {
            let twide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
            let mut info = SHFILEINFOW::default();
            let ok = SHGetFileInfoW(
                PCWSTR(twide.as_ptr()),
                Default::default(),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if ok != 0 && !info.hIcon.is_invalid() {
                let result = icon_to_bgra(info.hIcon);
                let _ = DestroyIcon(info.hIcon);
                if result.is_some() {
                    return result;
                }
            }
        }
        {
            use windows::Win32::UI::Shell::{SHDefExtractIconW, SHGFI_ICONLOCATION};
            let mut loc = SHFILEINFOW::default();
            let ok = SHGetFileInfoW(
                PCWSTR(wide.as_ptr()),
                Default::default(),
                Some(&mut loc),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICONLOCATION,
            );
            if ok != 0 && loc.szDisplayName[0] != 0 {
                let mut large = HICON::default();
                // 48px, packed in the low word as SHDefExtractIcon expects.
                if SHDefExtractIconW(
                    PCWSTR(loc.szDisplayName.as_ptr()),
                    loc.iIcon,
                    0,
                    Some(&mut large),
                    None,
                    48,
                )
                .is_ok()
                    && !large.is_invalid()
                {
                    let result = icon_to_bgra(large);
                    let _ = DestroyIcon(large);
                    if result.is_some() {
                        return result;
                    }
                }
            }
        }
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
