//! API keys in Windows Credential Manager (per user, encrypted by Windows).
//! Never written to settings.json or logs.

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

fn target(name: &str) -> Vec<u16> {
    format!("NOTCH/ai/{name}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

pub fn store(name: &str, secret: &str) -> bool {
    let mut target = target(name);
    let mut user: Vec<u16> = "notch".encode_utf16().chain(std::iter::once(0)).collect();
    let blob = secret.as_bytes();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user.as_mut_ptr()),
        ..Default::default()
    };
    unsafe { CredWriteW(&credential, 0).is_ok() }
}

pub fn load(name: &str) -> Option<String> {
    let target = target(name);
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(
            PCWSTR(target.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
        .ok()?;
        let cred = &*credential;
        let bytes =
            std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize);
        let secret = String::from_utf8_lossy(bytes).trim().to_string();
        CredFree(credential as *const core::ffi::c_void);
        (!secret.is_empty()).then_some(secret)
    }
}

pub fn delete(name: &str) -> bool {
    let target = target(name);
    unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None).is_ok() }
}
