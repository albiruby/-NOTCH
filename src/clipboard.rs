//! Text clipboard history. Kept in memory only: nothing leaves the process
//! and nothing is written to disk.

use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

const MAX_ITEMS: usize = 8;

static ITEMS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn slot() -> &'static Mutex<Vec<String>> {
    ITEMS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn start(hwnd: HWND) {
    unsafe {
        let _ = AddClipboardFormatListener(hwnd);
    }
    ingest(hwnd);
}

pub fn snapshot() -> Vec<String> {
    slot().lock().map(|items| items.clone()).unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut items) = slot().lock() {
        items.clear();
    }
}

/// Reads the clipboard and prepends the text if it is new.
pub fn ingest(hwnd: HWND) {
    let Some(text) = read_text(hwnd) else {
        return;
    };
    let text = text.trim().to_string();
    if text.is_empty() || text.len() > 4096 {
        return;
    }
    if let Ok(mut items) = slot().lock() {
        if items.first().map(|item| item == &text).unwrap_or(false) {
            return;
        }
        items.retain(|item| item != &text);
        items.insert(0, text);
        items.truncate(MAX_ITEMS);
    }
}

pub fn copy(hwnd: HWND, text: &str) {
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        if let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
            let ptr = GlobalLock(handle) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(handle);
                let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(handle.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

fn read_text(hwnd: HWND) -> Option<String> {
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return None;
        }
        let handle: HANDLE = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };
        let global = HGLOBAL(handle.0);
        let ptr = GlobalLock(global) as *const u16;
        let text = if ptr.is_null() {
            None
        } else {
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
                if len > 8192 {
                    break;
                }
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            Some(String::from_utf16_lossy(slice))
        };
        let _ = GlobalUnlock(global);
        let _ = CloseClipboard();
        text
    }
}
