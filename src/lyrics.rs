//! Synced lyrics from LRCLIB.
//!
//! A worker thread polls the media snapshot while the panel is open, fetches
//! lyrics once per track, and stores parsed LRC lines for the renderer.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::HWND;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_FLAG_SECURE,
};
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
    let host_w: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
    let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let session = WinHttpOpen(
            w!("NOTCH/1.0"),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return None;
        }
        let connect = WinHttpConnect(session, PCWSTR::from_raw(host_w.as_ptr()), 443, 0);
        if connect.is_null() {
            let _ = WinHttpCloseHandle(session);
            return None;
        }
        let request = WinHttpOpenRequest(
            connect,
            w!("GET"),
            PCWSTR::from_raw(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if request.is_null() {
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            return None;
        }

        let mut body = Vec::new();
        let sent = WinHttpSendRequest(request, None, None, 0, 0, 0);
        if sent.is_ok()
            && WinHttpReceiveResponse(request, std::ptr::null_mut()).is_ok()
        {
            loop {
                let mut available = 0u32;
                if WinHttpQueryDataAvailable(request, &mut available).is_err()
                    || available == 0
                {
                    break;
                }
                let mut chunk = vec![0u8; available as usize];
                let mut read = 0u32;
                if WinHttpReadData(
                    request,
                    chunk.as_mut_ptr() as *mut core::ffi::c_void,
                    available,
                    &mut read,
                )
                .is_err()
                {
                    break;
                }
                if read == 0 {
                    break;
                }
                chunk.truncate(read as usize);
                body.extend_from_slice(&chunk);
                if body.len() > 2 * 1024 * 1024 {
                    break;
                }
            }
        }

        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connect);
        let _ = WinHttpCloseHandle(session);

        if body.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&body).to_string())
        }
    }
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
