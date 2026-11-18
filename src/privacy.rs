//! Camera and microphone in-use indicator.
//!
//! Windows records every app's camera/microphone session under
//! `HKCU\...\CapabilityAccessManager\ConsentStore\{webcam,microphone}`. A
//! session that has a start time but no stop time (`LastUsedTimeStop == 0`)
//! is live. Reading a few registry keys every two seconds is cheap and needs
//! no permissions.

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Privacy {
    /// Apps using the microphone right now, by display name.
    pub mic: Vec<String>,
    /// Apps using the camera right now.
    pub cam: Vec<String>,
}

impl Privacy {
    pub fn active(&self) -> bool {
        !self.mic.is_empty() || !self.cam.is_empty()
    }

    /// Short label for the pill, e.g. "CAMERA · Zoom" or "MIC · Discord".
    pub fn label(&self) -> String {
        if let Some(app) = self.cam.first() {
            format!("CAMERA  ·  {app}")
        } else if let Some(app) = self.mic.first() {
            format!("MIC  ·  {app}")
        } else {
            String::new()
        }
    }
}

const ROOT: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\";

pub fn sample() -> Privacy {
    Privacy {
        mic: in_use("microphone"),
        cam: in_use("webcam"),
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn in_use(capability: &str) -> Vec<String> {
    let mut apps = Vec::new();
    let base = format!("{ROOT}{capability}");
    unsafe {
        if let Some(key) = open(HKEY_CURRENT_USER, &base) {
            scan(key, false, &mut apps);
            if let Some(non_packaged) = open(key, "NonPackaged") {
                scan(non_packaged, true, &mut apps);
                let _ = RegCloseKey(non_packaged);
            }
            let _ = RegCloseKey(key);
        }
    }
    apps.sort();
    apps.dedup();
    apps
}

unsafe fn open(parent: HKEY, path: &str) -> Option<HKEY> {
    let path = wide(path);
    let mut key = HKEY::default();
    let status = RegOpenKeyExW(parent, PCWSTR(path.as_ptr()), Some(0), KEY_READ, &mut key);
    (status == ERROR_SUCCESS).then_some(key)
}

unsafe fn scan(key: HKEY, non_packaged: bool, apps: &mut Vec<String>) {
    let mut index = 0;
    loop {
        let mut name = [0u16; 512];
        let mut len = name.len() as u32;
        let status = RegEnumKeyExW(
            key,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut len,
            None,
            None,
            None,
            None,
        );
        if status != ERROR_SUCCESS {
            break;
        }
        index += 1;
        let subkey = String::from_utf16_lossy(&name[..len as usize]);
        if subkey == "NonPackaged" {
            continue;
        }
        if let Some(child) = open(key, &subkey) {
            let start = qword(child, "LastUsedTimeStart");
            let stop = qword(child, "LastUsedTimeStop");
            let _ = RegCloseKey(child);
            if start.unwrap_or(0) > 0 && stop == Some(0) {
                apps.push(display_name(&subkey, non_packaged));
            }
        }
    }
}

unsafe fn qword(key: HKEY, value: &str) -> Option<u64> {
    let value = wide(value);
    let mut data = 0u64;
    let mut size = std::mem::size_of::<u64>() as u32;
    let status = RegQueryValueExW(
        key,
        PCWSTR(value.as_ptr()),
        None,
        None,
        Some(&mut data as *mut u64 as *mut u8),
        Some(&mut size),
    );
    (status == ERROR_SUCCESS).then_some(data)
}

/// `C:#Program Files#Zoom#bin#Zoom.exe` -> `Zoom`;
/// `Microsoft.WindowsCamera_8wekyb3d8bbwe` -> `WindowsCamera`.
fn display_name(key: &str, non_packaged: bool) -> String {
    if non_packaged {
        let file = key.rsplit('#').next().unwrap_or(key);
        file.strip_suffix(".exe")
            .or_else(|| file.strip_suffix(".EXE"))
            .unwrap_or(file)
            .to_string()
    } else {
        let family = key.split('_').next().unwrap_or(key);
        family.rsplit('.').next().unwrap_or(family).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::display_name;

    #[test]
    fn names() {
        assert_eq!(
            display_name("C:#Program Files#Zoom#bin#Zoom.exe", true),
            "Zoom"
        );
        assert_eq!(
            display_name("Microsoft.WindowsCamera_8wekyb3d8bbwe", false),
            "WindowsCamera"
        );
    }
}
