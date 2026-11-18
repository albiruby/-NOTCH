//! CPU and memory sampling. Cheap, and only runs while the notch is expanded.

use windows::Win32::Foundation::FILETIME;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

/// IF_TYPE_SOFTWARE_LOOPBACK: skip the loopback interface.
const IF_TYPE_LOOPBACK: u32 = 24;

pub struct SysMon {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    pub cpu: f32,
    pub ram: f32,
    prev_rx: u64,
    prev_tx: u64,
    prev_net: Option<std::time::Instant>,
    /// Download / upload throughput, bytes per second.
    pub rx_bps: f64,
    pub tx_bps: f64,
}

impl SysMon {
    pub fn new() -> Self {
        let mut mon = Self {
            prev_idle: 0,
            prev_kernel: 0,
            prev_user: 0,
            cpu: 0.0,
            ram: 0.0,
            prev_rx: 0,
            prev_tx: 0,
            prev_net: None,
            rx_bps: 0.0,
            tx_bps: 0.0,
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

        self.sample_net();
    }

    fn sample_net(&mut self) {
        let (rx, tx) = net_octets();
        let now = std::time::Instant::now();
        if let Some(prev) = self.prev_net {
            let secs = now.duration_since(prev).as_secs_f64();
            if secs > 0.05 {
                self.rx_bps = rx.saturating_sub(self.prev_rx) as f64 / secs;
                self.tx_bps = tx.saturating_sub(self.prev_tx) as f64 / secs;
            }
        }
        self.prev_rx = rx;
        self.prev_tx = tx;
        self.prev_net = Some(now);
    }
}

/// Total received/transmitted bytes across non-loopback interfaces.
fn net_octets() -> (u64, u64) {
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return (0, 0);
        }
        let count = (*table).NumEntries as usize;
        let rows = (*table).Table.as_ptr();
        let (mut rx, mut tx) = (0u64, 0u64);
        for i in 0..count {
            let row = &*rows.add(i);
            if row.Type == IF_TYPE_LOOPBACK {
                continue;
            }
            rx = rx.saturating_add(row.InOctets);
            tx = tx.saturating_add(row.OutOctets);
        }
        FreeMibTable(table as *const core::ffi::c_void);
        (rx, tx)
    }
}

/// Human-readable rate, e.g. "1.2 MB/s" or "840 KB/s".
pub fn format_rate(bps: f64) -> String {
    if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else if bps >= 1000.0 {
        format!("{:.0} KB/s", bps / 1000.0)
    } else {
        format!("{:.0} B/s", bps.max(0.0))
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
