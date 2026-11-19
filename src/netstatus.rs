//! Wi-Fi and Bluetooth radio state through WinRT.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::Devices::Radios::{Radio, RadioKind, RadioState};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows_future::{AsyncStatus, IAsyncOperation};

pub const WM_APP_NET: u32 = WM_APP + 5;

#[derive(Clone, Copy, Default)]
pub struct NetStatus {
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
}

static STATUS: OnceLock<Mutex<NetStatus>> = OnceLock::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);
static WAKE: AtomicIsize = AtomicIsize::new(0);

fn slot() -> &'static Mutex<NetStatus> {
    STATUS.get_or_init(|| Mutex::new(NetStatus::default()))
}

pub fn start(hwnd: HWND) {
    WAKE.store(hwnd.0 as isize, Ordering::Relaxed);
    thread::Builder::new()
        .name("notch-net".into())
        .spawn(worker)
        .ok();
}

pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

pub fn snapshot() -> NetStatus {
    slot().lock().map(|s| *s).unwrap_or_default()
}

fn block<T: Clone + windows::core::RuntimeType>(
    op: &IAsyncOperation<T>,
) -> windows::core::Result<T> {
    while op.Status()? == AsyncStatus::Started {
        thread::sleep(Duration::from_millis(4));
    }
    op.GetResults()
}

fn worker() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let mut last = NetStatus::default();
    loop {
        if ACTIVE.load(Ordering::Relaxed) {
            let next = read();
            let changed = next.wifi != last.wifi || next.bluetooth != last.bluetooth;
            if changed {
                if let Ok(mut slot) = slot().lock() {
                    *slot = next;
                }
                last = next;
                wake();
            }
        }
        thread::sleep(Duration::from_millis(2000));
    }
}

fn read() -> NetStatus {
    let mut status = NetStatus::default();
    let Ok(radios) = Radio::GetRadiosAsync().and_then(|op| block(&op)) else {
        return status;
    };
    let Ok(size) = radios.Size() else {
        return status;
    };
    for index in 0..size {
        let Ok(radio) = radios.GetAt(index) else {
            continue;
        };
        let kind = radio.Kind().unwrap_or(RadioKind::Other);
        let on = radio.State().map(|s| s == RadioState::On).unwrap_or(false);
        match kind {
            RadioKind::WiFi => status.wifi = Some(on),
            RadioKind::Bluetooth => status.bluetooth = Some(on),
            _ => {}
        }
    }
    status
}

/// Flip the Wi-Fi (or Bluetooth) radio on a worker thread. Returns false when
/// there is no such radio, so the caller can open Settings instead.
pub fn toggle(wifi: bool) -> bool {
    let current = snapshot();
    let state = if wifi { current.wifi } else { current.bluetooth };
    let Some(on) = state else {
        return false;
    };
    let target = if on { RadioState::Off } else { RadioState::On };
    // Show the new state at once; the worker confirms it afterwards.
    if let Ok(mut slot) = slot().lock() {
        if wifi {
            slot.wifi = Some(!on);
        } else {
            slot.bluetooth = Some(!on);
        }
    }
    wake();
    thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let want = if wifi { RadioKind::WiFi } else { RadioKind::Bluetooth };
        let _ = Radio::RequestAccessAsync().and_then(|op| block(&op));
        if let Ok(radios) = Radio::GetRadiosAsync().and_then(|op| block(&op)) {
            for radio in radios {
                if radio.Kind().ok() == Some(want) {
                    let _ = radio.SetStateAsync(target).and_then(|op| block(&op));
                }
            }
        }
        let next = read();
        if let Ok(mut slot) = slot().lock() {
            *slot = next;
        }
        wake();
    });
    true
}

fn wake() {
    let raw = WAKE.load(Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(raw as *mut core::ffi::c_void)),
                WM_APP_NET,
                Default::default(),
                Default::default(),
            );
        }
    }
}
