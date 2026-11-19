//! Minimal HTTPS over WinHTTP, shared by lyrics, weather, the AI cockpit
//! and Ask. Supports GET/POST and reading the body as it arrives.

use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

/// Body size cap for buffered requests.
const MAX_BODY: usize = 2 * 1024 * 1024;

/// GET `https://{host}{path}` with extra request headers. Returns the HTTP
/// status code and body (capped at 2 MB), or None on a transport failure.
pub fn get(host: &str, path: &str, headers: &[(&str, &str)]) -> Option<(u32, String)> {
    buffered("GET", host, path, headers, None)
}

fn buffered(
    method: &str,
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Option<(u32, String)> {
    let mut out = Vec::new();
    let status = stream(method, host, path, headers, body, |chunk| {
        out.extend_from_slice(chunk);
        out.len() <= MAX_BODY
    })?;
    Some((status, String::from_utf8_lossy(&out).to_string()))
}

/// Send a request and hand the body to `on_chunk` as it arrives. Return
/// false from `on_chunk` to stop reading. Returns the status code, or None
/// on a transport failure.
pub fn stream(
    method: &str,
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    mut on_chunk: impl FnMut(&[u8]) -> bool,
) -> Option<u32> {
    let host_w: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
    let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let method_w: Vec<u16> = method.encode_utf16().chain(std::iter::once(0)).collect();
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
            PCWSTR::from_raw(method_w.as_ptr()),
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
        let headers_arg = if header_w.is_empty() {
            None
        } else {
            Some(header_w.as_slice())
        };
        let (body_ptr, body_len) = match body {
            Some(b) => (Some(b.as_ptr() as *const core::ffi::c_void), b.len() as u32),
            None => (None, 0),
        };
        let ok = WinHttpSendRequest(request, headers_arg, body_ptr, body_len, body_len, 0).is_ok()
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
                if !on_chunk(&chunk) {
                    break;
                }
            }
        }

        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connect);
        let _ = WinHttpCloseHandle(session);
        ok.then_some(status)
    }
}
