//! Tray icon and context menu.

use windows::core::{w, Result};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, PostMessageW,
    SetForegroundWindow, TrackPopupMenu, IDI_APPLICATION, MF_CHECKED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, TPM_RIGHTBUTTON, WM_NULL,
};

pub const WM_TRAY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 3;
pub const TRAY_ID: u32 = 1;

pub const CMD_TOGGLE: usize = 1;
pub const CMD_SETTINGS: usize = 2;
pub const CMD_QUIT: usize = 3;
pub const CMD_MODE_NOTCH: usize = 4;
pub const CMD_MODE_ISLAND: usize = 5;
pub const CMD_AUTOSTART: usize = 6;
pub const CMD_OPACITY_80: usize = 7;
pub const CMD_OPACITY_90: usize = 8;
pub const CMD_OPACITY_100: usize = 9;
pub const CMD_FULLSCREEN: usize = 10;
pub const CMD_LAYOUT_PANEL: usize = 11;
pub const CMD_LAYOUT_WIDE: usize = 12;
pub const CMD_LAYOUT_CARDS: usize = 13;
pub const CMD_LAYOUT_HUB: usize = 14;
pub const CMD_RESET_POSITION: usize = 15;
pub const CMD_MODE_NOTCH_X: usize = 16;
pub const CMD_MODE_ISLAND_PRO: usize = 17;
pub const CMD_THEME_VIOLET: usize = 18;
pub const CMD_THEME_MINT: usize = 19;
pub const CMD_THEME_CORAL: usize = 20;
pub const CMD_MASCOT: usize = 23;
pub const CMD_ASK: usize = 25;
pub const CMD_OCR: usize = 26;
pub const CMD_MINIMAL: usize = 24;

pub struct MenuState {
    pub mode: crate::state::SurfaceMode,
    pub theme: crate::theme::accent::Theme,
    pub autostart: bool,
    pub opacity: f32,
    pub hide_on_fullscreen: bool,
    pub layout: crate::state::Layout,
    pub mascot: bool,
    pub minimal: bool,
}

pub struct Tray {
    data: NOTIFYICONDATAW,
}

impl Tray {
    pub fn new(hwnd: HWND) -> Result<Self> {
        unsafe {
            let mut data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                ..Default::default()
            };
            // Resource 1 is the NOTCH logo embedded by build.rs.
            let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
            data.hIcon = match LoadIconW(Some(module.into()), windows::core::PCWSTR(1 as _)) {
                Ok(icon) => icon,
                Err(_) => LoadIconW(None, IDI_APPLICATION)?,
            };
            set_text(&mut data.szTip, "NOTCH");
            if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                return Err(windows::core::Error::from_thread());
            }
            Ok(Self { data })
        }
    }

    #[allow(dead_code)]
    pub fn set_tip(&mut self, text: &str) {
        set_text(&mut self.data.szTip, text);
        self.data.uFlags = NIF_TIP;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &self.data);
        }
    }

    pub fn remove(&self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.remove();
    }
}

pub fn show_menu(owner: HWND, state: &MenuState) -> Result<()> {
    unsafe {
        let menu = CreatePopupMenu()?;
        AppendMenuW(menu, MF_STRING, CMD_TOGGLE, w!("Show / hide notch"))?;
        AppendMenuW(menu, MF_STRING, CMD_ASK, w!("Ask NOTCH…"))?;
        AppendMenuW(menu, MF_STRING, CMD_OCR, w!("Copy text from screen (OCR)"))?;
        AppendMenuW(menu, MF_SEPARATOR, 0, w!(""))?;
        use crate::state::SurfaceMode;
        let check = |mode: SurfaceMode| {
            if state.mode == mode {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            }
        };
        AppendMenuW(
            menu,
            check(SurfaceMode::Notch),
            CMD_MODE_NOTCH,
            w!("Classic notch (glass)"),
        )?;
        AppendMenuW(
            menu,
            check(SurfaceMode::NotchX),
            CMD_MODE_NOTCH_X,
            w!("iPhone X notch"),
        )?;
        AppendMenuW(
            menu,
            check(SurfaceMode::Island),
            CMD_MODE_ISLAND,
            w!("Dynamic island (glass)"),
        )?;
        AppendMenuW(
            menu,
            check(SurfaceMode::IslandPro),
            CMD_MODE_ISLAND_PRO,
            w!("Dynamic Island (Pro)"),
        )?;
        AppendMenuW(
            menu,
            MF_STRING,
            CMD_RESET_POSITION,
            w!("Reset island position"),
        )?;

        let layout_menu = CreatePopupMenu()?;
        let panel_flags = if state.layout == crate::state::Layout::Panel {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let wide_flags = if state.layout == crate::state::Layout::Wide {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let cards_flags = if state.layout == crate::state::Layout::Cards {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let hub_flags = if state.layout == crate::state::Layout::Hub {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(
            layout_menu,
            hub_flags,
            CMD_LAYOUT_HUB,
            w!("Hub (click to open all)"),
        )?;
        AppendMenuW(layout_menu, panel_flags, CMD_LAYOUT_PANEL, w!("Panel"))?;
        AppendMenuW(layout_menu, wide_flags, CMD_LAYOUT_WIDE, w!("Wide bar"))?;
        AppendMenuW(layout_menu, cards_flags, CMD_LAYOUT_CARDS, w!("Cards"))?;
        AppendMenuW(menu, MF_POPUP, layout_menu.0 as usize, w!("Layout"))?;

        let themes = CreatePopupMenu()?;
        {
            use crate::theme::accent::Theme;
            for (id, label, value) in [
                (CMD_THEME_VIOLET, w!("Violet"), Theme::Violet),
                (CMD_THEME_MINT, w!("Aurora Mint (fresh)"), Theme::Mint),
                (CMD_THEME_CORAL, w!("Sunset Coral"), Theme::Coral),
            ] {
                let flags = if state.theme == value {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                };
                AppendMenuW(themes, flags, id, label)?;
            }
        }
        AppendMenuW(menu, MF_POPUP, themes.0 as usize, w!("Theme"))?;

        let glass = CreatePopupMenu()?;
        for (id, label, value) in [
            (CMD_OPACITY_80, w!("80%"), 0.8f32),
            (CMD_OPACITY_90, w!("90%"), 0.9f32),
            (CMD_OPACITY_100, w!("100%"), 1.0f32),
        ] {
            let flags = if (state.opacity - value).abs() < 0.02 {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            AppendMenuW(glass, flags, id, label)?;
        }
        AppendMenuW(menu, MF_POPUP, glass.0 as usize, w!("Glass opacity"))?;

        let start_flags = if state.autostart {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(menu, start_flags, CMD_AUTOSTART, w!("Start with Windows"))?;
        let fullscreen_flags = if state.hide_on_fullscreen {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(
            menu,
            fullscreen_flags,
            CMD_FULLSCREEN,
            w!("Hide on fullscreen"),
        )?;
        let flag = |on: bool| if on { MF_STRING | MF_CHECKED } else { MF_STRING };
        AppendMenuW(menu, MF_SEPARATOR, 0, w!(""))?;
        AppendMenuW(menu, flag(state.mascot), CMD_MASCOT, w!("Idle mascot"))?;
        AppendMenuW(menu, flag(state.minimal), CMD_MINIMAL, w!("Minimal notch (thin line)"))?;
        AppendMenuW(menu, MF_SEPARATOR, 0, w!(""))?;
        AppendMenuW(menu, MF_STRING, CMD_SETTINGS, w!("Open settings folder"))?;
        AppendMenuW(menu, MF_SEPARATOR, 0, w!(""))?;
        AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Quit NOTCH"))?;

        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(owner);
        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, None, owner, None);
        let _ = PostMessageW(Some(owner), WM_NULL, Default::default(), Default::default());
        let _ = DestroyMenu(menu);
    }
    Ok(())
}

pub fn autostart_enabled() -> bool {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    };
    unsafe {
        let mut key = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let mut size = 0u32;
        let exists = RegQueryValueExW(key, w!("Notch"), None, None, None, Some(&mut size)).is_ok()
            && size > 2;
        let _ = RegCloseKey(key);
        exists
    }
}

pub fn set_autostart(enabled: bool) -> bool {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };
    unsafe {
        let mut key = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let ok = if enabled {
            let exe = std::env::current_exe().unwrap_or_default();
            let quoted = format!("\"{}\"", exe.display());
            let wide: Vec<u16> = quoted.encode_utf16().chain(std::iter::once(0)).collect();
            RegSetValueExW(
                key,
                w!("Notch"),
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide.as_ptr() as *const u8,
                    wide.len() * 2,
                )),
            )
            .is_ok()
        } else {
            RegDeleteValueW(key, w!("Notch")).is_ok()
        };
        let _ = RegCloseKey(key);
        ok
    }
}

fn set_text(dst: &mut [u16], text: &str) {
    let mut i = 0;
    for unit in text.encode_utf16() {
        if i >= dst.len() - 1 {
            break;
        }
        dst[i] = unit;
        i += 1;
    }
    while i < dst.len() {
        dst[i] = 0;
        i += 1;
    }
}
