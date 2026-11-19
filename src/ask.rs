//! Ask: Ctrl+Alt+Space opens a small prompt under the notch.
//!
//! - Ask anything: answers stream in from Google Gemini, using the user's
//!   own key (stored in Windows Credential Manager as `NOTCH/ai/gemini`).
//! - Tab switches to Translate (detects the language, translates).
//! - A line ending in `=` is quick math or currency ("100usd to idr="),
//!   answered locally without Gemini.
//!
//! The window is a plain GDI popup so it stays independent of the notch's
//! composition surface and can take keyboard focus normally.

use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::Mutex;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_APP_ASK: u32 = WM_APP + 40;
const WIDTH: i32 = 640;
const PAD: i32 = 18;
const INPUT_TOP: i32 = 40;
const INPUT_H: i32 = 36;
const ANSWER_TOP: i32 = INPUT_TOP + INPUT_H + 14;
const MAX_ANSWER_H: i32 = 460;
/// Gemini model; the `-latest` alias tracks Google's current Flash model.
const MODEL: &str = "gemini-flash-latest";

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Ask,
    Translate,
}

#[derive(Default)]
struct State {
    input: String,
    answer: String,
    mode: Mode,
    busy: bool,
    error: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static HWND_ASK: AtomicIsize = AtomicIsize::new(0);
/// Bumped per request so a stale stream stops writing into a new answer.
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default))
}

fn hwnd() -> HWND {
    HWND(HWND_ASK.load(Ordering::Relaxed) as *mut core::ffi::c_void)
}

fn refresh() {
    let h = hwnd();
    if !h.is_invalid() {
        unsafe {
            let _ = PostMessageW(Some(h), WM_APP_ASK, WPARAM(0), LPARAM(0));
        }
    }
}

/// `notch key set gemini <key>` / `notch key clear gemini`.
pub fn cli(args: &[String]) -> Option<i32> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["key", "set", "gemini", key] => {
            let ok = crate::secrets::store("gemini", key);
            println!("{}", if ok { "Saved Gemini key to Windows Credential Manager." } else { "Could not save the key." });
            Some(if ok { 0 } else { 1 })
        }
        ["key", "clear", "gemini"] => {
            crate::secrets::delete("gemini");
            println!("Removed Gemini key.");
            Some(0)
        }
        _ => None,
    }
}

/// Show the prompt (or hide it if it is already in front).
pub fn toggle() {
    unsafe {
        let mut h = hwnd();
        if h.is_invalid() {
            h = match create() {
                Some(h) => h,
                None => return,
            };
        }
        if IsWindowVisible(h).as_bool() && GetForegroundWindow() == h {
            let _ = ShowWindow(h, SW_HIDE);
            return;
        }
        place(h);
        let _ = ShowWindow(h, SW_SHOW);
        force_foreground(h);
        let _ = InvalidateRect(Some(h), None, true);
    }
}

/// Take keyboard focus even when opened from the tray or another app.
/// Windows refuses SetForegroundWindow from a background process unless
/// it shares the foreground thread's input queue for the moment.
pub unsafe fn force_foreground(h: HWND) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    let foreground = GetForegroundWindow();
    let their = GetWindowThreadProcessId(foreground, None);
    let ours = GetCurrentThreadId();
    let attached = their != 0 && their != ours && AttachThreadInput(ours, their, true).as_bool();
    let _ = BringWindowToTop(h);
    let _ = SetForegroundWindow(h);
    let _ = SetFocus(Some(h));
    if attached {
        let _ = AttachThreadInput(ours, their, false);
    }
}

unsafe fn create() -> Option<HWND> {
    let instance = GetModuleHandleW(None).ok()?;
    let class = WNDCLASSW {
        style: CS_DROPSHADOW,
        lpfnWndProc: Some(proc),
        hInstance: instance.into(),
        lpszClassName: w!("NotchAsk"),
        hCursor: LoadCursorW(None, IDC_IBEAM).unwrap_or_default(),
        ..Default::default()
    };
    RegisterClassW(&class);
    let h = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        w!("NotchAsk"),
        w!("Ask NOTCH"),
        WS_POPUP,
        0,
        0,
        WIDTH,
        ANSWER_TOP,
        None,
        None,
        Some(instance.into()),
        None,
    )
    .ok()?;
    HWND_ASK.store(h.0 as isize, Ordering::Relaxed);
    Some(h)
}

/// Centre under the notch on the monitor with the cursor; height fits the
/// answer.
unsafe fn place(h: HWND) {
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = GetMonitorInfoW(monitor, &mut info);
    let work = info.rcWork;
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(h).max(96) as i32;
    let scale = |v: i32| v * dpi / 96;
    let width = scale(WIDTH);
    let height = scale(ANSWER_TOP) + answer_height(h) + scale(PAD);
    let x = work.left + (work.right - work.left - width) / 2;
    let y = work.top + scale(64);
    let _ = SetWindowPos(h, Some(HWND_TOPMOST), x, y, width, height, SWP_NOACTIVATE);
    let rgn = CreateRoundRectRgn(0, 0, width + 1, height + 1, scale(28), scale(28));
    SetWindowRgn(h, Some(rgn), true);
}

unsafe fn font(h: HWND, size: i32, weight: i32) -> HFONT {
    face(h, size, weight, w!("Inter"))
}

/// A bundled face by name (registered for GDI in `create`).
unsafe fn face(h: HWND, size: i32, weight: i32, name: PCWSTR) -> HFONT {
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(h).max(96) as i32;
    CreateFontW(
        -(size * dpi / 96),
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        0,
        name,
    )
}

/// Pixel height the answer text needs at the current width.
unsafe fn answer_height(h: HWND) -> i32 {
    let text = with(|s| s.answer.clone());
    if text.is_empty() {
        return 0;
    }
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(h).max(96) as i32;
    let dc = GetDC(Some(h));
    let f = font(h, 15, 400);
    let old = SelectObject(dc, f.into());
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut r = RECT {
        left: 0,
        top: 0,
        right: (WIDTH - PAD * 2) * dpi / 96,
        bottom: 0,
    };
    DrawTextW(dc, &mut wide, &mut r, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL);
    SelectObject(dc, old);
    let _ = DeleteObject(f.into());
    ReleaseDC(Some(h), dc);
    (r.bottom - r.top).min(MAX_ANSWER_H * dpi / 96)
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

unsafe fn paint(h: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let dc = BeginPaint(h, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(h, &mut client);
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(h).max(96) as i32;
    let sc = |v: i32| v * dpi / 96;

    // Double buffer.
    let mem = CreateCompatibleDC(Some(dc));
    let bmp = CreateCompatibleBitmap(dc, client.right, client.bottom);
    let old_bmp = SelectObject(mem, bmp.into());

    let bg = CreateSolidBrush(rgb(0x12, 0x0F, 0x1C));
    FillRect(mem, &client, bg);
    let _ = DeleteObject(bg.into());
    SetBkMode(mem, TRANSPARENT);

    let (input, answer, mode, busy, error) =
        with(|s| (s.input.clone(), s.answer.clone(), s.mode, s.busy, s.error));
    let need_key = input.starts_with("AIza");

    // Header.
    let small = face(h, 12, 700, w!("Red Hat Display"));
    let old_font = SelectObject(mem, small.into());
    SetTextColor(mem, rgb(0xA7, 0x8B, 0xFA));
    let title = match (need_key, mode) {
        (true, _) => "GEMINI KEY",
        (_, Mode::Ask) => "ASK NOTCH",
        (_, Mode::Translate) => "TRANSLATE",
    };
    let mut r = RECT { left: sc(PAD), top: sc(14), right: client.right - sc(PAD), bottom: sc(32) };
    let mut t: Vec<u16> = title.encode_utf16().collect();
    DrawTextW(mem, &mut t, &mut r, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
    SetTextColor(mem, rgb(0x7A, 0x74, 0x8C));
    let hint = if busy { "thinking…" } else { "Enter send · Tab translate · Esc close · Ctrl+C copy" };
    let mut t: Vec<u16> = hint.encode_utf16().collect();
    DrawTextW(mem, &mut t, &mut r, DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX);

    // Input field.
    let field = RECT { left: sc(PAD - 6), top: sc(INPUT_TOP), right: client.right - sc(PAD - 6), bottom: sc(INPUT_TOP + INPUT_H) };
    let field_brush = CreateSolidBrush(rgb(0x22, 0x1D, 0x30));
    let rgn = CreateRoundRectRgn(field.left, field.top, field.right, field.bottom, sc(14), sc(14));
    let _ = FillRgn(mem, rgn, field_brush);
    let _ = DeleteObject(rgn.into());
    let _ = DeleteObject(field_brush.into());
    let big = font(h, 17, 400);
    SelectObject(mem, big.into());
    let shown = if input.is_empty() {
        SetTextColor(mem, rgb(0x6B, 0x65, 0x7D));
        match (need_key, mode) {
            (true, _) => "AIza…".to_string(),
            (_, Mode::Ask) => "Ask anything, or 12*7=  ·  100usd to idr=".to_string(),
            (_, Mode::Translate) => "Text to translate".to_string(),
        }
    } else {
        SetTextColor(mem, rgb(0xF4, 0xF1, 0xFA));
        if need_key {
            format!("{}▏", "•".repeat(input.chars().count().min(40)))
        } else {
            format!("{input}▏")
        }
    };
    let mut r = RECT { left: field.left + sc(12), top: field.top, right: field.right - sc(12), bottom: field.bottom };
    let mut t: Vec<u16> = shown.encode_utf16().collect();
    // Keep the end (caret) visible for long input.
    DrawTextW(mem, &mut t, &mut r, DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_PATH_ELLIPSIS);

    // Answer.
    if !answer.is_empty() {
        let body = font(h, 15, 400);
        SelectObject(mem, body.into());
        SetTextColor(mem, if error { rgb(0xF8, 0x9A, 0x8C) } else { rgb(0xE6, 0xE1, 0xF0) });
        let mut r = RECT { left: sc(PAD), top: sc(ANSWER_TOP), right: client.right - sc(PAD), bottom: client.bottom - sc(PAD) };
        let mut t: Vec<u16> = answer.encode_utf16().collect();
        DrawTextW(mem, &mut t, &mut r, DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL | DT_END_ELLIPSIS);
        let _ = DeleteObject(body.into());
    }

    SelectObject(mem, old_font);
    let _ = DeleteObject(small.into());
    let _ = DeleteObject(big.into());
    let _ = BitBlt(dc, 0, 0, client.right, client.bottom, Some(mem), 0, 0, SRCCOPY);
    SelectObject(mem, old_bmp);
    let _ = DeleteObject(bmp.into());
    let _ = DeleteDC(mem);
    let _ = EndPaint(h, &ps);
}

fn submit() {
    let (input, mode) = with(|s| (s.input.trim().to_string(), s.mode));
    // A pasted Gemini key (they start with "AIza") is saved, not asked.
    let need_key = input.starts_with("AIza");
    if input.is_empty() {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let set = |answer: String, busy: bool, error: bool| {
        with(|s| {
            s.answer = answer;
            s.busy = busy;
            s.error = error;
        });
        refresh();
    };

    // Quick math / currency, no key needed.
    if let Some(expr) = input.strip_suffix('=') {
        if let Some((amount, from, to)) = crate::calc::parse_currency(expr) {
            set(format!("Converting {from} → {to}…"), true, false);
            std::thread::spawn(move || {
                let text = match crate::calc::rate(&from, &to) {
                    Some(rate) => format!(
                        "{} {from} = {} {to}\n1 {from} = {} {to} (ECB reference rate via Frankfurter)",
                        crate::calc::format_number(amount),
                        crate::calc::format_number(amount * rate),
                        crate::calc::format_number(rate)
                    ),
                    None => format!("Couldn't get a {from}→{to} rate (offline, or an unsupported currency)."),
                };
                if GENERATION.load(Ordering::Relaxed) == generation {
                    set(text, false, false);
                }
            });
            return;
        }
        match crate::calc::eval(expr) {
            Some(v) => set(format!("= {}", crate::calc::format_number(v)), false, false),
            None => set("That isn't a sum I can work out.".into(), false, true),
        }
        return;
    }

    // First use: the input is the key. Check it before saving.
    if need_key {
        set("Checking the key…".into(), true, false);
        let key = input.clone();
        std::thread::spawn(move || {
            let path = "/v1beta/models?pageSize=1";
            let result = crate::http::get("generativelanguage.googleapis.com", path, &[("x-goog-api-key", &key)]);
            let (text, ok) = match result {
                Some((200, _)) if crate::secrets::store("gemini", &key) => ("Key saved. Ask away.".to_string(), true),
                Some((200, _)) => ("The key works, but Credential Manager refused to save it.".to_string(), false),
                Some((status, body)) => (format!("Gemini rejected the key ({status}). {}", error_message(&body)), false),
                None => ("Can't reach Gemini. Check the internet connection and try again.".to_string(), false),
            };
            with(|s| {
                if ok {
                    s.input.clear();
                }
            });
            set(text, false, !ok);
        });
        return;
    }

    let Some(key) = crate::secrets::load("gemini") else {
        set(
            "Gemini isn't set up yet. When you're ready, paste your free key from aistudio.google.com here (it starts with AIza) and press Enter. Math (12*7=) and currency (100usd to idr=) work without it."
                .into(),
            false,
            false,
        );
        return;
    };
    set(String::new(), true, false);
    std::thread::spawn(move || stream_answer(&key, &input, mode, generation));
}

fn system_prompt(mode: Mode) -> &'static str {
    match mode {
        Mode::Ask => "You are NOTCH, a concise assistant that lives in the Windows notch. \
                      Answer briefly in plain text (no Markdown, no tables), in the same language as the question.",
        Mode::Translate => "You are a translator. Detect the language of the user's text. If it is not English, \
                            translate it to English; if it is English, translate it to Indonesian. \
                            First line: '<detected language> → <target language>'. Then only the translation.",
    }
}

fn error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error")?.get("message")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Text pieces from one SSE `data:` JSON chunk of streamGenerateContent.
pub fn sse_text(json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return String::new();
    };
    value
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| parts.iter().filter_map(|p| p.get("text")?.as_str()).collect())
        .unwrap_or_default()
}

fn stream_answer(key: &str, input: &str, mode: Mode, generation: u64) {
    let body = serde_json::json!({
        "systemInstruction": { "parts": [{ "text": system_prompt(mode) }] },
        "contents": [{ "role": "user", "parts": [{ "text": input }] }],
    })
    .to_string();
    let path = format!("/v1beta/models/{MODEL}:streamGenerateContent?alt=sse");
    let mut pending = String::new();
    let mut raw = String::new();
    let status = crate::http::stream(
        "POST",
        "generativelanguage.googleapis.com",
        &path,
        &[("Content-Type", "application/json"), ("x-goog-api-key", key)],
        Some(body.as_bytes()),
        |chunk| {
            if GENERATION.load(Ordering::Relaxed) != generation {
                return false;
            }
            let text = String::from_utf8_lossy(chunk);
            raw.push_str(&text);
            pending.push_str(&text);
            // SSE events end with a blank line.
            while let Some(end) = pending.find("\n\n").or_else(|| pending.find("\r\n\r\n")) {
                let event: String = pending.drain(..end).collect();
                pending = pending.trim_start_matches(['\r', '\n']).to_string();
                for line in event.lines() {
                    if let Some(data) = line.trim().strip_prefix("data:") {
                        let piece = sse_text(data.trim());
                        if !piece.is_empty() {
                            with(|s| s.answer.push_str(&piece));
                            refresh();
                        }
                    }
                }
            }
            true
        },
    );
    if GENERATION.load(Ordering::Relaxed) != generation {
        return;
    }
    let (message, error) = match status {
        Some(200) => (None, false),
        Some(400) if raw.contains("API_KEY_INVALID") => (Some("Gemini says the key is invalid. Paste a new one here (it starts with AIza).".to_string()), true),
        Some(429) => (Some("Gemini is rate-limiting this key right now. Try again in a minute.".to_string()), true),
        Some(503) => (Some("Gemini is busy right now. Try again shortly.".to_string()), true),
        Some(code) => (Some(format!("Gemini returned {code}. {}", error_message(&raw))), true),
        None => (Some("Can't reach Gemini (offline or blocked).".to_string()), true),
    };
    with(|s| {
        s.busy = false;
        if let Some(m) = message {
            s.answer = m;
            s.error = error;
            if raw.contains("API_KEY_INVALID") {
                crate::secrets::delete("gemini");
            }
        } else if s.answer.trim().is_empty() {
            s.answer = "(No answer.)".into();
        }
    });
    refresh();
}

fn clipboard_text() -> Option<String> {
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        OpenClipboard(Some(hwnd())).ok()?;
        let text = GetClipboardData(CF_UNICODETEXT).ok().and_then(|handle| {
            let global = windows::Win32::Foundation::HGLOBAL(handle.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(global);
            Some(s)
        });
        let _ = CloseClipboard();
        text
    }
}

unsafe extern "system" fn proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(h);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_APP_ASK => {
            place(h);
            let _ = InvalidateRect(Some(h), None, false);
            LRESULT(0)
        }
        WM_ACTIVATE => {
            // Like a launcher: clicking elsewhere puts it away.
            if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE {
                let _ = ShowWindow(h, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_CHAR => {
            let ch = wparam.0 as u32;
            let mut dirty = true;
            match ch {
                0x0D => submit(),
                0x1B => {
                    let _ = ShowWindow(h, SW_HIDE);
                    dirty = false;
                }
                0x09 => with(|s| {
                    s.mode = if s.mode == Mode::Ask { Mode::Translate } else { Mode::Ask };
                }),
                0x08 => with(|s| {
                    s.input.pop();
                }),
                // Ctrl+Backspace: delete the last word.
                0x7F => with(|s| {
                    let trimmed = s.input.trim_end().to_string();
                    let cut = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
                    s.input.truncate(cut);
                }),
                // Ctrl+V
                0x16 => {
                    if let Some(text) = clipboard_text() {
                        let line = text.replace(['\r', '\n'], " ");
                        with(|s| s.input.push_str(line.trim()));
                    }
                }
                // Ctrl+C copies the answer.
                0x03 => {
                    let answer = with(|s| s.answer.clone());
                    if !answer.is_empty() {
                        crate::clipboard::copy(h, &answer);
                    }
                    dirty = false;
                }
                _ => {
                    if let Some(c) = char::from_u32(ch).filter(|c| !c.is_control()) {
                        with(|s| {
                            if s.input.chars().count() < 2000 {
                                s.input.push(c);
                            }
                        });
                    } else {
                        dirty = false;
                    }
                }
            }
            if dirty {
                let _ = InvalidateRect(Some(h), None, false);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(h, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::sse_text;

    #[test]
    fn reads_stream_chunks() {
        let chunk = r#"{"candidates":[{"content":{"parts":[{"text":"Hello"},{"text":" world"}],"role":"model"}}]}"#;
        assert_eq!(sse_text(chunk), "Hello world");
        assert_eq!(sse_text("{}"), "");
        assert_eq!(sse_text("not json"), "");
    }
}
