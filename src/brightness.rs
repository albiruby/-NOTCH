//! Internal display brightness through Dxva2.

use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};

pub struct Brightness {
    monitor: HANDLE,
    min: u32,
    max: u32,
    physical: Vec<PHYSICAL_MONITOR>,
}

impl Brightness {
    pub fn new(hwnd: HWND) -> Option<Self> {
        unsafe {
            let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut count = 0;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut count).is_err() || count == 0 {
                return None;
            }
            let mut physical = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(hmonitor, &mut physical).is_err() {
                return None;
            }
            let monitor = physical[0].hPhysicalMonitor;
            let mut min = 0;
            let mut current = 0;
            let mut max = 0;
            if GetMonitorBrightness(monitor, &mut min, &mut current, &mut max) == 0 || max <= min {
                let _ = DestroyPhysicalMonitors(&physical);
                return None;
            }
            Some(Self {
                monitor,
                min,
                max,
                physical,
            })
        }
    }

    pub fn level(&self) -> f32 {
        unsafe {
            let mut min = 0;
            let mut current = 0;
            let mut max = 0;
            if GetMonitorBrightness(self.monitor, &mut min, &mut current, &mut max) == 0 {
                return 0.0;
            }
            ((current.saturating_sub(min)) as f32 / (max.saturating_sub(min)).max(1) as f32)
                .clamp(0.0, 1.0)
        }
    }

    pub fn set(&self, level: f32) {
        let span = self.max.saturating_sub(self.min) as f32;
        let value = self.min + (level.clamp(0.0, 1.0) * span) as u32;
        unsafe {
            let _ = SetMonitorBrightness(self.monitor, value);
        }
    }
}

impl Drop for Brightness {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyPhysicalMonitors(&self.physical);
        }
    }
}
