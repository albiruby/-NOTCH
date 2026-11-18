//! Synced lyrics from LRCLIB.
//!
//! A worker thread polls the media snapshot while the panel is open, fetches
//! lyrics once per track, and stores parsed LRC lines for the renderer.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_LYRICS: u32 = WM_APP + 8;

#[derive(Clone, Default)]
pub struct Line {
    pub at: f32,
    pub text: String,
}

#[derive(Clone, Default)]
pub struct Lyrics {
    pub lines: Vec<Line>,
}

impl Lyrics {
    pub fn index_at(&self, position: f32) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let mut found = 0;
        for (index, line) in self.lines.iter().enumerate() {
            if line.at <= position + 0.15 {
                found = index;
            } else {
                break;
            }
        }
        Some(found)
    }
}

static LYRICS: OnceLock<Mutex<Lyrics>> = OnceLock::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);
static WAKE: AtomicIsize = AtomicIsize::new(0);

fn slot() -> &'static Mutex<Lyrics> {
    LYRICS.get_or_init(|| Mutex::new(Lyrics::default()))
}

pub fn start(hwnd: HWND) {
    WAKE.store(hwnd.0 as isize, Ordering::Relaxed);
    thread::Builder::new()
        .name("notch-lyrics".into())
        .spawn(worker)
        .ok();
}

pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

pub fn snapshot() -> Lyrics {
    slot().lock().map(|lyrics| lyrics.clone()).unwrap_or_default()
}

fn worker() {
    let mut key = String::new();
    let mut cache: Vec<(String, Lyrics)> = Vec::new();

    loop {
        if ACTIVE.load(Ordering::Relaxed) {
            let media = crate::media::snapshot();
            let next_key = if media.available
                && !media.title.is_empty()
                && media.title != "Unknown track"
            {
                format!("{}\u{1}{}", media.artist, media.title)
            } else {
                String::new()
            };

            if next_key != key {
                key = next_key;
                let cached = cache
                    .iter()
                    .find(|(k, _)| k == &key)
                    .map(|(_, lyrics)| lyrics.clone());
                let lyrics = if key.is_empty() {
                    Lyrics::default()
                } else if let Some(found) = cached {
                    found
                } else {
                    // Do not show the previous track's lines while the new
                    // request is in flight.
                    if let Ok(mut slot) = slot().lock() {
                        *slot = Lyrics::default();
                    }
                    wake();
                    let fetched = fetch(&media.artist, &media.title).unwrap_or_default();
                    if !fetched.lines.is_empty() {
                        cache.push((key.clone(), fetched.clone()));
                        if cache.len() > 12 {
                            cache.remove(0);
                        }
                    }
                    fetched
                };
                crate::log(&format!("lyrics: key={key:?} lines={}", lyrics.lines.len()));
                if let Ok(mut slot) = slot().lock() {
                    *slot = lyrics;
                }
                wake();
            }
        }
        thread::sleep(Duration::from_millis(1200));
    }
}

fn fetch(artist: &str, title: &str) -> Option<Lyrics> {
    let path = format!(
        "/api/search?artist_name={}&track_name={}",
        encode(artist),
        encode(title)
    );
    let body = http_get("lrclib.net", &path)?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    let items = value.as_array()?;
    for item in items {
        if let Some(synced) = item.get("syncedLyrics").and_then(|v| v.as_str()) {
            let lines = parse_lrc(synced);
            if !lines.is_empty() {
                return Some(Lyrics { lines });
            }
        }
    }
    None
}

fn parse_lrc(raw: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for row in raw.lines() {
        let row = row.trim();
        if row.is_empty() {
            continue;
        }
        let mut rest = row;
        let mut stamps = Vec::new();
        loop {
            if !rest.starts_with('[') {
                break;
            }
            let Some(end) = rest.find(']') else {
                break;
            };
            let inner = &rest[1..end];
            if let Some(at) = timestamp(inner) {
                stamps.push(at);
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
        let text = rest.trim();
        if text.is_empty() {
            continue;
        }
        for at in stamps {
            lines.push(Line {
                at,
                text: text.to_string(),
            });
        }
    }
    lines.sort_by(|a, b| a.at.partial_cmp(&b.at).unwrap_or(std::cmp::Ordering::Equal));
    lines.dedup_by(|a, b| a.at == b.at && a.text == b.text);
    lines
}

fn timestamp(raw: &str) -> Option<f32> {
    let mut parts = raw.split(':');
    let minutes: f32 = parts.next()?.parse().ok()?;
    let seconds: f32 = parts.next()?.parse().ok()?;
    Some(minutes * 60.0 + seconds)
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn http_get(host: &str, path: &str) -> Option<String> {
    crate::http::get(host, path, &[])
        .filter(|(status, body)| (200..300).contains(status) && !body.is_empty())
        .map(|(_, body)| body)
}

fn wake() {
    let raw = WAKE.load(Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(raw as *mut core::ffi::c_void)),
                WM_APP_LYRICS,
                Default::default(),
                Default::default(),
            );
        }
    }
}
