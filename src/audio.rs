//! Master volume through WASAPI, with change notifications.
//!
//! Hardware volume keys are handled by Windows itself, so the endpoint
//! callback is the only reliable way to see them. The callback only posts a
//! message; the UI thread reads the new level.

use std::ptr;

use windows::core::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eConsole, eRender, AUDIO_VOLUME_NOTIFICATION_DATA, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_VOLUME: u32 = WM_APP + 6;

#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct VolumeWatcher {
    hwnd: isize,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeWatcher_Impl {
    fn OnNotify(&self, _data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
        let raw = self.hwnd;
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(raw as *mut core::ffi::c_void)),
                    WM_APP_VOLUME,
                    Default::default(),
                    Default::default(),
                );
            }
        }
        Ok(())
    }
}

pub struct Audio {
    volume: IAudioEndpointVolume,
    _callback: IAudioEndpointVolumeCallback,
}

impl Audio {
    pub fn new(hwnd: HWND) -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            let watcher: IAudioEndpointVolumeCallback = VolumeWatcher {
                hwnd: hwnd.0 as isize,
            }
            .into();
            let _ = volume.RegisterControlChangeNotify(&watcher);
            Ok(Self {
                volume,
                _callback: watcher,
            })
        }
    }

    pub fn level(&self) -> f32 {
        unsafe {
            self.volume
                .GetMasterVolumeLevelScalar()
                .unwrap_or(0.0)
        }
    }

    pub fn set_level(&self, level: f32) {
        unsafe {
            let _ = self
                .volume
                .SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), ptr::null());
        }
    }

    pub fn muted(&self) -> bool {
        unsafe { self.volume.GetMute().map(|m| m.as_bool()).unwrap_or(false) }
    }

    pub fn set_muted(&self, muted: bool) {
        unsafe {
            let _ = self.volume.SetMute(muted, ptr::null());
        }
    }
}
