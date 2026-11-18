//! Window placement, DPI and fullscreen policy.

use windows::core::Result;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn scale_for(hwnd: HWND) -> f32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        if dpi == 0 {
            1.0
        } else {
            dpi as f32 / 96.0
        }
    }
}

/// Places a surface at the bottom-right of the monitor work area.
pub fn place_right(
    hwnd: HWND,
    width_px: i32,
    height_px: i32,
    margin_px: i32,
) -> Result<()> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(monitor, &mut info).ok()?;
        let work = info.rcWork;
        let x = work.right - width_px - margin_px;
        let y = work.bottom - height_px - margin_px;
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width_px,
            height_px,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }
    Ok(())
}

/// True when a fullscreen app, presentation or D3D fullscreen owns the screen.
///
/// `QUNS_BUSY` alone is too broad (it also fires for the lock screen and
/// elevation prompts), so it only counts when the foreground window really
/// covers the whole monitor.
pub fn should_hide() -> bool {
    let state = notification_state();
    if state == 3 || state == 4 {
        return true;
    }
    if state != 2 {
        return false;
    }
    foreground_covers_monitor()
}

pub fn notification_state() -> i32 {
    unsafe {
        match SHQueryUserNotificationState() {
            Ok(state) => state.0,
            Err(_) => -1,
        }
    }
}

fn foreground_covers_monitor() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == std::process::id() {
            return false;
        }

        let mut class = [0u16; 64];
        let len = GetClassNameW(fg, &mut class);
        let name = String::from_utf16_lossy(&class[..len as usize]);
        if matches!(name.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
            return false;
        }

        let mut rect = RECT::default();
        if GetWindowRect(fg, &mut rect).is_err() {
            return false;
        }

        let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;

        rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
    }
}


/// Monitor and work-area rectangles (physical pixels) for the window's monitor.
pub fn monitor_rects(hwnd: HWND) -> (RECT, RECT) {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        (info.rcMonitor, info.rcWork)
    }
}

/// Top-left of the window in screen pixels.
pub fn origin(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        (rect.left, rect.top)
    }
}

/// Moves the window (topmost, no activation) to `x, y` with the given size.
pub fn move_to(hwnd: HWND, x: i32, y: i32, width_px: i32, height_px: i32) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width_px,
            height_px,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}
