//! Current weather from wttr.in (no API key; location auto-detected by IP).
//! Refreshed every 30 minutes on a worker thread.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_WEATHER: u32 = WM_APP + 22;
const INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub temp_c: i32,
    pub feels_c: i32,
    pub desc: String,
    pub place: String,
    pub ok: bool,
    /// Coordinates of the detected area.
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

impl Weather {
    /// e.g. "12°C · Partly cloudy · Jakarta".
    pub fn line(&self) -> String {
        if !self.ok {
            return String::new();
        }
        if self.place.is_empty() {
            format!("{}°C · {}", self.temp_c, self.desc)
        } else {
            format!("{}°C · {} · {}", self.temp_c, self.desc, self.place)
        }
    }

    /// A Segoe MDL2 glyph roughly matching the description.
    pub fn glyph(&self) -> &'static str {
        let d = self.desc.to_ascii_lowercase();
        if d.contains("thunder") {
            "\u{E70F}"
        } else if d.contains("snow") || d.contains("sleet") || d.contains("blizzard") {
            "\u{E80F}"
        } else if d.contains("rain") || d.contains("drizzle") || d.contains("shower") {
            "\u{E9C6}"
        } else if d.contains("fog")
            || d.contains("mist")
            || d.contains("cloud")
            || d.contains("overcast")
        {
            "\u{E9CA}"
        } else {
            "\u{E706}"
        }
    }
}

/// Parse a wttr.in `?format=j1` body.
pub fn parse(body: &str) -> Option<Weather> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let current = value.get("current_condition")?.as_array()?.first()?;
    let temp = current.get("temp_C")?.as_str()?.parse().ok()?;
    let feels = current
        .get("FeelsLikeC")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(temp);
    let desc = current
        .get("weatherDesc")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|d| d.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let area = value
        .get("nearest_area")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first());
    let coord = |key: &str| {
        area.and_then(|a| a.get(key))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
    };
    let place = value
        .get("nearest_area")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|area| area.get("areaName"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|n| n.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    Some(Weather {
        temp_c: temp,
        feels_c: feels,
        desc,
        place,
        ok: true,
        lat: coord("latitude"),
        lon: coord("longitude"),
    })
}

fn slot() -> &'static Mutex<Weather> {
    static SLOT: OnceLock<Mutex<Weather>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(Weather::default()))
}

static TARGET: AtomicIsize = AtomicIsize::new(0);

pub fn start(hwnd: HWND) {
    TARGET.store(hwnd.0 as isize, Ordering::Relaxed);
    let _ = thread::Builder::new()
        .name("notch-weather".into())
        .spawn(|| loop {
            if let Some((status, body)) = crate::http::get("wttr.in", "/?format=j1", &[]) {
                if (200..300).contains(&status) {
                    if let Some(weather) = parse(&body) {
                        if let Ok(mut guard) = slot().lock() {
                            *guard = weather;
                        }
                        notify();
                    }
                }
            }
            thread::sleep(INTERVAL);
        });
}

pub fn snapshot() -> Weather {
    slot().lock().map(|w| w.clone()).unwrap_or_default()
}

fn notify() {
    let raw = TARGET.load(Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(raw as *mut core::ffi::c_void)),
                WM_APP_WEATHER,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        let body = r#"{
          "current_condition":[{"temp_C":"12","FeelsLikeC":"10","weatherDesc":[{"value":"Partly cloudy"}]}],
          "nearest_area":[{"areaName":[{"value":"Jakarta"}]}]
        }"#;
        let w = parse(body).unwrap();
        assert_eq!(w.temp_c, 12);
        assert_eq!(w.feels_c, 10);
        assert_eq!(w.line(), "12°C · Partly cloudy · Jakarta");
        assert_eq!(w.glyph(), "\u{E9CA}");
    }
}
