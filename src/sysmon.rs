//! CPU and memory sampling. Cheap, and only runs while the notch is expanded.

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

pub struct SysMon {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    pub cpu: f32,
    pub ram: f32,
}

impl SysMon {
    pub fn new() -> Self {
        let mut mon = Self {
            prev_idle: 0,
            prev_kernel: 0,
            prev_user: 0,
            cpu: 0.0,
            ram: 0.0,
        };
        mon.sample();
        mon
    }

    pub fn sample(&mut self) {
        let (idle, kernel, user) = system_times();
        if self.prev_kernel != 0 || self.prev_user != 0 || self.prev_idle != 0 {
            let d_idle = idle.saturating_sub(self.prev_idle);
            let d_kernel = kernel.saturating_sub(self.prev_kernel);
            let d_user = user.saturating_sub(self.prev_user);
            let total = d_kernel + d_user;
            if total > 0 {
                let busy = total.saturating_sub(d_idle);
                self.cpu = (busy as f64 / total as f64 * 100.0) as f32;
            }
        }
        self.prev_idle = idle;
        self.prev_kernel = kernel;
        self.prev_user = user;

        unsafe {
            let mut status = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };
            if GlobalMemoryStatusEx(&mut status).is_ok() {
                self.ram = status.dwMemoryLoad as f32;
            }
        }
    }
}

fn system_times() -> (u64, u64, u64) {
    unsafe {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_err() {
            return (0, 0, 0);
        }
        (to_u64(idle), to_u64(kernel), to_u64(user))
    }
}

fn to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}
