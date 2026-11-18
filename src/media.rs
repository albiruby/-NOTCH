//! Now-playing metadata from the Windows media session.
//!
//! The WinRT calls block, so they run on a worker thread. The UI reads a
//! snapshot and never touches COM directly.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::core::Result;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows_future::{AsyncStatus, IAsyncOperation};

pub const WM_APP_MEDIA: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 4;

fn block<T: Clone + windows::core::RuntimeType>(
    op: &IAsyncOperation<T>,
) -> windows::core::Result<T> {
    while op.Status()? == AsyncStatus::Started {
        thread::sleep(Duration::from_millis(4));
    }
    op.GetResults()
}

#[derive(Clone, Default)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub available: bool,
    pub position: f32,
    pub duration: f32,
    pub art: Option<std::sync::Arc<Vec<u8>>>,
    pub art_id: u64,
}

#[derive(Clone, Copy)]
pub enum Command {
    Toggle,
    Next,
    Previous,
    Seek(f32),
}

static INFO: OnceLock<Mutex<MediaInfo>> = OnceLock::new();
static SENDER: OnceLock<Mutex<Option<std::sync::mpsc::Sender<Command>>>> = OnceLock::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);
static WAKE: AtomicIsize = AtomicIsize::new(0);

fn store() -> &'static Mutex<MediaInfo> {
    INFO.get_or_init(|| Mutex::new(MediaInfo::default()))
}

pub fn start(hwnd: HWND) {
    WAKE.store(hwnd.0 as isize, Ordering::Relaxed);
    thread::Builder::new()
        .name("notch-media".into())
        .spawn(worker)
        .ok();
}

/// Sampling only runs while the expanded panel is visible.
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

pub fn snapshot() -> MediaInfo {
    store().lock().map(|info| info.clone()).unwrap_or_default()
}

pub fn send(cmd: Command) {
    if let Some(slot) = SENDER.get() {
        if let Ok(guard) = slot.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(cmd);
            }
        }
    }
}

fn worker() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let manager = loop {
        match Manager::RequestAsync().and_then(|op| block(&op)) {
            Ok(manager) => break manager,
            Err(_) => thread::sleep(Duration::from_secs(2)),
        }
    };

    let mut last = MediaInfo::default();
    let (tx, rx) = std::sync::mpsc::channel::<Command>();
    let _ = SENDER.set(Mutex::new(Some(tx)));

    loop {
        while let Ok(cmd) = rx.try_recv() {
            run_command(&manager, cmd);
        }

        if ACTIVE.load(Ordering::Relaxed) {
            let next = read(&manager, &last).unwrap_or_default();
            if changed(&last, &next) {
                crate::log(&format!(
                    "media update: {:?} / {:?} playing={} available={} art={}",
                    next.title,
                    next.artist,
                    next.playing,
                    next.available,
                    next.art.is_some()
                ));
                if let Ok(mut slot) = store().lock() {
                    *slot = next.clone();
                }
                last = next;
                wake();
            }
        }
        thread::sleep(Duration::from_millis(1000));
    }
}

fn read(manager: &Manager, last: &MediaInfo) -> Result<MediaInfo> {
    let session = manager.GetCurrentSession()?;
    let props = block(&session.TryGetMediaPropertiesAsync()?)?;
    let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
    let playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .map(|status| status == PlaybackStatus::Playing)
        .unwrap_or(false);

    let title = if title.is_empty() {
        "Unknown track".to_string()
    } else {
        title
    };

    let new_track = title != last.title;
    let art = if new_track {
        thumbnail(&props)
    } else {
        last.art.clone()
    };

    let (position, duration) = match session.GetTimelineProperties() {
        Ok(timeline) => {
            let start = timeline.StartTime().map(|v| v.Duration).unwrap_or(0);
            let end = timeline.EndTime().map(|v| v.Duration).unwrap_or(0);
            let pos = timeline.Position().map(|v| v.Duration).unwrap_or(0);
            (
                ((pos - start).max(0) as f64 / 10_000_000.0) as f32,
                ((end - start).max(0) as f64 / 10_000_000.0) as f32,
            )
        }
        Err(_) => (0.0, 0.0),
    };

    Ok(MediaInfo {
        title,
        artist,
        playing,
        available: true,
        position,
        duration,
        art_id: if new_track {
            art.as_ref().map(|_| crate::state::next_id()).unwrap_or(0)
        } else {
            last.art_id
        },
        art,
    })
}

/// Reads the session thumbnail into memory. Decoding happens on the UI thread
/// because Direct2D bitmaps are device bound.
fn thumbnail(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<std::sync::Arc<Vec<u8>>> {
    use windows::Storage::Streams::DataReader;

    let reference = props.Thumbnail().ok()?;
    let stream = block(&reference.OpenReadAsync().ok()?).ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 16 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    block(&reader.LoadAsync(size).ok()?).ok()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(std::sync::Arc::new(bytes))
}

fn run_command(manager: &Manager, cmd: Command) {
    let Ok(session) = manager.GetCurrentSession() else {
        return;
    };
    match cmd {
        Command::Toggle => {
            let _ = session.TryTogglePlayPauseAsync().and_then(|op| block(&op));
        }
        Command::Next => {
            let _ = session.TrySkipNextAsync().and_then(|op| block(&op));
        }
        Command::Previous => {
            let _ = session.TrySkipPreviousAsync().and_then(|op| block(&op));
        }
        Command::Seek(seconds) => {
            let position = (seconds.max(0.0) * 10_000_000.0) as i64;
            let _ = session
                .TryChangePlaybackPositionAsync(position)
                .and_then(|op| block(&op));
        }
    }
}

fn changed(a: &MediaInfo, b: &MediaInfo) -> bool {
    a.title != b.title
        || a.artist != b.artist
        || a.playing != b.playing
        || a.available != b.available
        || a.art_id != b.art_id
        || (a.position as i32) != (b.position as i32)
}

fn wake() {
    let raw = WAKE.load(Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(HWND(raw as *mut core::ffi::c_void)),
                WM_APP_MEDIA,
                Default::default(),
                Default::default(),
            );
        }
    }
}
