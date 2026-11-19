//! Small companion features: to-do reminders, quick
//! peeks for the charger, the internet connection and the keyboard
//! language, and the idle mascot. Everything here is local; it only reads
//! state Windows already exposes.

use windows::Win32::Foundation::SYSTEMTIME;

/// Something the notch should briefly say.
pub enum Event {
    /// Text for the pill, and how long to show it (ms).
    Message(String, u32),
}

/// Mascot state for the compact pill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Eyes {
    #[default]
    Hidden,
    Open,
    Blink,
    Dozing,
}

#[derive(Default)]
pub struct Companion {
    reminded: Vec<(String, String)>,
    last_charging: Option<bool>,
    last_online: Option<bool>,
    last_lang: Option<u16>,
    last_audio: Option<String>,
    pub eyes: Eyes,
}

fn day_key(t: &SYSTEMTIME) -> String {
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

fn minutes(t: &SYSTEMTIME) -> f64 {
    t.wHour as f64 * 60.0 + t.wMinute as f64 + t.wSecond as f64 / 60.0
}

/// A task's reminder time: "14:30 Reply to Dina" or "Reply to Dina @14:30".
pub fn task_time(text: &str) -> Option<(u32, u32, String)> {
    let parse = |s: &str| {
        let (h, m) = s.split_once(':')?;
        let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
        (h < 24 && m < 60 && s.len() <= 5).then_some((h, m))
    };
    let text = text.trim();
    if let Some((first, rest)) = text.split_once(' ') {
        if let Some((h, m)) = parse(first) {
            return Some((h, m, rest.trim().to_string()));
        }
    }
    if let Some((rest, last)) = text.rsplit_once('@') {
        if let Some((h, m)) = parse(last.trim()) {
            return Some((h, m, rest.trim().to_string()));
        }
    }
    None
}

impl Companion {
    /// Called every couple of seconds with the current state; returns what
    /// to announce.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        now: &SYSTEMTIME,
        tasks: &[crate::tasks::Task],
        battery: Option<u32>,
        charging: bool,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        let today = day_key(now);
        let now_min = minutes(now);

        // To-do reminders.
        for task in tasks.iter().filter(|t| !t.done) {
            if let Some((h, m, label)) = task_time(&task.text) {
                let at = (h * 60 + m) as f64;
                let key = (today.clone(), task.text.clone());
                if now_min >= at && now_min < at + 2.0 && !self.reminded.contains(&key) {
                    self.reminded.push(key);
                    out.push(Event::Message(format!("TO-DO  {label}"), 6000));
                }
            }
        }
        self.reminded.retain(|(day, _)| *day == today);

        // Charger plugged in.
        if let Some(was) = self.last_charging {
            if charging && !was {
                let pct = battery.map(|b| format!("  {b}%")).unwrap_or_default();
                out.push(Event::Message(format!("CHARGING{pct}"), 2500));
            } else if !charging && was {
                let pct = battery.map(|b| format!("  {b}%")).unwrap_or_default();
                out.push(Event::Message(format!("ON BATTERY{pct}"), 2000));
            }
        }
        self.last_charging = Some(charging);

        // Internet lost / back.
        let online = is_online();
        if let Some(was) = self.last_online {
            if was && !online {
                out.push(Event::Message("NO INTERNET".into(), 3000));
            } else if !was && online {
                out.push(Event::Message("BACK ONLINE".into(), 2000));
            }
        }
        self.last_online = Some(online);

        // Default playback device changed (headphones connected / removed).
        if let Some(name) = default_audio_name() {
            if self.last_audio.is_none() {
                crate::log(&format!("audio device: {name}"));
            }
            if self.last_audio.as_ref().is_some_and(|old| *old != name) {
                out.push(Event::Message(audio_label(&name), 2500));
            }
            self.last_audio = Some(name);
        }
        out
    }

    /// Keyboard language of the foreground window; a message when it changes.
    pub fn keyboard(&mut self) -> Option<Event> {
        let lang = foreground_lang()?;
        let changed = self.last_lang.is_some_and(|old| old != lang);
        self.last_lang = Some(lang);
        changed.then(|| Event::Message(format!("KEYBOARD  {}", lang_name(lang)), 1500))
    }

    /// Update the mascot from idle time. Returns true when it changed.
    pub fn update_eyes(&mut self, enabled: bool, busy: bool, tick: u64) -> bool {
        let idle = idle_seconds();
        let next = if !enabled || busy || idle < 60 {
            Eyes::Hidden
        } else if idle >= 300 {
            Eyes::Dozing
        } else if tick % 8 == 0 {
            // A blink every few seconds (tick is ~500 ms).
            Eyes::Blink
        } else {
            Eyes::Open
        };
        let changed = next != self.eyes;
        self.eyes = next;
        changed
    }
}

/// Friendly name of the default playback device, e.g. "AirBuds Pro".
pub fn default_audio_name() -> Option<String> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
    // PKEY_Device_FriendlyName.
    const FRIENDLY: PROPERTYKEY = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = devices.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let mut value = store.GetValue(&FRIENDLY).ok()?;
        let text = PropVariantToStringAlloc(&value).ok();
        let _ = PropVariantClear(&mut value);
        let text = text?;
        let name = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as *const _));
        name.filter(|n| !n.is_empty())
    }
}

/// Short label for an audio device change.
pub fn audio_label(name: &str) -> String {
    let lower = name.to_lowercase();
    let kind = if ["headphone", "headset", "earbud", "airpods", "buds", "hands-free", "bluetooth"]
        .iter()
        .any(|k| lower.contains(k))
    {
        "HEADPHONES"
    } else {
        "AUDIO"
    };
    // Drop the "(…)" driver suffix Windows adds, keep the device name.
    let short = name.split(" (").next().unwrap_or(name).trim();
    format!("{kind}  {short}")
}

fn idle_seconds() -> u32 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !GetLastInputInfo(&mut info).as_bool() {
            return 0;
        }
        GetTickCount().wrapping_sub(info.dwTime) / 1000
    }
}

fn is_online() -> bool {
    use windows::Win32::Networking::WinInet::{InternetGetConnectedState, INTERNET_CONNECTION};
    unsafe {
        let mut flags = INTERNET_CONNECTION(0);
        InternetGetConnectedState(&mut flags, None).is_ok()
    }
}

fn foreground_lang() -> Option<u16> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let thread = GetWindowThreadProcessId(hwnd, None);
        let hkl = GetKeyboardLayout(thread);
        Some((hkl.0 as usize & 0xFFFF) as u16)
    }
}

/// "EN-US", "ID-ID", … for a language id.
fn lang_name(lang: u16) -> String {
    use windows::Win32::Globalization::LCIDToLocaleName;
    let mut buf = [0u16; 85];
    let len = unsafe { LCIDToLocaleName(lang as u32, Some(&mut buf), 0) };
    if len <= 1 {
        return format!("{lang:04X}");
    }
    String::from_utf16_lossy(&buf[..len as usize - 1]).to_uppercase()
}

/// Monday-first week around `now`: (weekday letter, day of month, is today).
pub fn week(now: &SYSTEMTIME) -> Vec<(char, u32, bool)> {
    let days_in = |y: i32, m: u32| match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    };
    // wDayOfWeek: 0 = Sunday.
    let from_monday = (now.wDayOfWeek as i32 + 6) % 7;
    let (mut y, mut m, mut d) = (now.wYear as i32, now.wMonth as u32, now.wDay as i32 - from_monday);
    while d < 1 {
        m = if m == 1 { 12 } else { m - 1 };
        if m == 12 {
            y -= 1;
        }
        d += days_in(y, m) as i32;
    }
    let mut out = Vec::with_capacity(7);
    let mut d = d as u32;
    for (i, letter) in ['M', 'T', 'W', 'T', 'F', 'S', 'S'].into_iter().enumerate() {
        out.push((letter, d, i as i32 == from_monday));
        d += 1;
        if d > days_in(y, m) {
            d = 1;
            m = if m == 12 { 1 } else { m + 1 };
            if m == 1 {
                y += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_times() {
        assert_eq!(task_time("14:30 Reply to Dina"), Some((14, 30, "Reply to Dina".into())));
        assert_eq!(task_time("Post update @9:05"), Some((9, 5, "Post update".into())));
        assert_eq!(task_time("Revise the report"), None);
        assert_eq!(task_time("25:00 nope"), None);
    }

    #[test]
    fn week_spans_month_end() {
        // Thursday 3 Dec 2026: week runs Mon 30 Nov .. Sun 6 Dec.
        let now = SYSTEMTIME {
            wYear: 2026,
            wMonth: 12,
            wDay: 3,
            wDayOfWeek: 4,
            ..Default::default()
        };
        let w = week(&now);
        assert_eq!(w.iter().map(|d| d.1).collect::<Vec<_>>(), vec![30, 1, 2, 3, 4, 5, 6]);
        assert!(w[3].2);
    }
}

#[cfg(test)]
mod audio_tests {
    use super::audio_label;

    #[test]
    fn labels() {
        assert_eq!(audio_label("AirBuds Pro (Bluetooth)"), "HEADPHONES  AirBuds Pro");
        assert_eq!(audio_label("Headphones (Realtek(R) Audio)"), "HEADPHONES  Headphones");
        assert_eq!(audio_label("Speakers (Realtek(R) Audio)"), "AUDIO  Speakers");
    }
}
