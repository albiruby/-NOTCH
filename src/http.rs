//! Minimal HTTPS GET over WinHTTP, shared by lyrics and the AI cockpit.

use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

/// GET `https://{host}{path}` with extra request headers. Returns the HTTP
/// status code and body (capped at 2 MB), or None on a transport failure.
pub fn get(host: &str, path: &str, headers: &[(&str, &str)]) -> Option<(u32, String)> {
    let host_w: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
    let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let header_block: String = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect();
    let header_w: Vec<u16> = header_block.encode_utf16().collect();
    unsafe {
        let session = WinHttpOpen(
            w!("NOTCH/1.0"),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return None;
        }
        let connect = WinHttpConnect(session, PCWSTR::from_raw(host_w.as_ptr()), 443, 0);
        if connect.is_null() {
            let _ = WinHttpCloseHandle(session);
            return None;
        }
        let request = WinHttpOpenRequest(
            connect,
            w!("GET"),
            PCWSTR::from_raw(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if request.is_null() {
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            return None;
        }

        let mut status = 0u32;
        let mut body = Vec::new();
        let headers_arg = if header_w.is_empty() { None } else { Some(header_w.as_slice()) };
        let ok = WinHttpSendRequest(request, headers_arg, None, 0, 0, 0).is_ok()
            && WinHttpReceiveResponse(request, std::ptr::null_mut()).is_ok();
        if ok {
            let mut size = std::mem::size_of::<u32>() as u32;
            let _ = WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut status as *mut u32 as *mut core::ffi::c_void),
                &mut size,
                std::ptr::null_mut(),
            );
            loop {
                let mut available = 0u32;
                if WinHttpQueryDataAvailable(request, &mut available).is_err() || available == 0 {
                    break;
                }
                let mut chunk = vec![0u8; available as usize];
                let mut read = 0u32;
                if WinHttpReadData(
                    request,
                    chunk.as_mut_ptr() as *mut core::ffi::c_void,
                    available,
                    &mut read,
                )
                .is_err()
                    || read == 0
                {
                    break;
                }
                chunk.truncate(read as usize);
                body.extend_from_slice(&chunk);
                if body.len() > 2 * 1024 * 1024 {
                    break;
                }
            }
        }

        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connect);
        let _ = WinHttpCloseHandle(session);
        ok.then(|| (status, String::from_utf8_lossy(&body).to_string()))
    }
}
