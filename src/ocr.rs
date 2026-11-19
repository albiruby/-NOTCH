//! Smart OCR: Ctrl+Alt+T freezes the screen, the user drags a box, and the
//! text inside is read with the OCR engine built into Windows (offline)
//! and copied to the clipboard.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Posted to the notch window when OCR finishes; read the text with `take`.
pub const WM_APP_OCR: u32 = WM_APP + 41;

struct Selection {
    /// Frozen screen and a dimmed copy (HBITMAP handles as isize).
    shot: isize,
    dim: isize,
    size: (i32, i32),
    start: Option<POINT>,
    current: POINT,
}

static SELECTION: Mutex<Option<Selection>> = Mutex::new(None);
static RESULT: Mutex<Option<Result<String, String>>> = Mutex::new(None);
static NOTIFY: AtomicIsize = AtomicIsize::new(0);

/// The finished OCR text (Ok) or a message for the user (Err).
pub fn take() -> Option<Result<String, String>> {
    RESULT.lock().ok()?.take()
}

/// Freeze the screen and let the user pick an area. `notify` gets
/// `WM_APP_OCR` when the text is ready.
pub fn start(notify: HWND) {
    NOTIFY.store(notify.0 as isize, Ordering::Relaxed);
    unsafe {
        if SELECTION.lock().map(|s| s.is_some()).unwrap_or(true) {
            return;
        }
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        // Capture before the overlay exists.
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let shot = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, shot.into());
        let _ = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT);
        SelectObject(mem, old);

        // Dimmed copy for the area outside the selection.
        let dim = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, dim.into());
        let src = CreateCompatibleDC(Some(screen));
        let old_src = SelectObject(src, shot.into());
        let _ = BitBlt(mem, 0, 0, w, h, Some(src), 0, 0, SRCCOPY);
        let black = CreateSolidBrush(COLORREF(0));
        let shade = CreateCompatibleDC(Some(screen));
        let one = CreateCompatibleBitmap(screen, 1, 1);
        let old_shade = SelectObject(shade, one.into());
        let r1 = RECT { left: 0, top: 0, right: 1, bottom: 1 };
        FillRect(shade, &r1, black);
        let blend = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: 120, AlphaFormat: 0 };
        let _ = AlphaBlend(mem, 0, 0, w, h, shade, 0, 0, 1, 1, blend);
        SelectObject(shade, old_shade);
        let _ = DeleteObject(one.into());
        let _ = DeleteDC(shade);
        let _ = DeleteObject(black.into());
        SelectObject(src, old_src);
        let _ = DeleteDC(src);
        SelectObject(mem, old);
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);

        if let Ok(mut slot) = SELECTION.lock() {
            *slot = Some(Selection {
                shot: shot.0 as isize,
                dim: dim.0 as isize,
                size: (w, h),
                start: None,
                current: POINT::default(),
            });
        }

        let Ok(instance) = GetModuleHandleW(None) else { return };
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: w!("NotchOcr"),
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&class);
        if let Ok(h) = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("NotchOcr"),
            w!("NOTCH OCR"),
            WS_POPUP,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(instance.into()),
            None,
        ) {
            let _ = ShowWindow(h, SW_SHOW);
            crate::ask::force_foreground(h);
        } else {
            finish(None);
        }
    }
}

fn norm(a: POINT, b: POINT) -> RECT {
    RECT { left: a.x.min(b.x), top: a.y.min(b.y), right: a.x.max(b.x), bottom: a.y.max(b.y) }
}

unsafe fn paint(h: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let dc = BeginPaint(h, &mut ps);
    if let Ok(guard) = SELECTION.lock() {
        if let Some(sel) = guard.as_ref() {
            let (w, hh) = sel.size;
            let back = CreateCompatibleDC(Some(dc));
            let frame = CreateCompatibleBitmap(dc, w, hh);
            let old_frame = SelectObject(back, frame.into());
            let src = CreateCompatibleDC(Some(dc));
            let old = SelectObject(src, HGDIOBJ(sel.dim as *mut _));
            let _ = BitBlt(back, 0, 0, w, hh, Some(src), 0, 0, SRCCOPY);
            if let Some(start) = sel.start {
                let r = norm(start, sel.current);
                SelectObject(src, HGDIOBJ(sel.shot as *mut _));
                let _ = BitBlt(back, r.left, r.top, r.right - r.left, r.bottom - r.top, Some(src), r.left, r.top, SRCCOPY);
                let pen = CreatePen(PS_SOLID, 2, COLORREF(0x00FA_8BA7));
                let old_pen = SelectObject(back, pen.into());
                let old_brush = SelectObject(back, GetStockObject(NULL_BRUSH));
                let _ = Rectangle(back, r.left, r.top, r.right, r.bottom);
                SelectObject(back, old_brush);
                SelectObject(back, old_pen);
                let _ = DeleteObject(pen.into());
            }
            // Hint at the top centre.
            SetBkMode(back, TRANSPARENT);
            SetTextColor(back, COLORREF(0x00FFFFFF));
            let mut r = RECT { left: 0, top: 24, right: w, bottom: 60 };
            let mut text: Vec<u16> = "Drag over the text to copy it  ·  Esc to cancel".encode_utf16().collect();
            DrawTextW(back, &mut text, &mut r, DT_CENTER | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(src, old);
            let _ = DeleteDC(src);
            let _ = BitBlt(dc, 0, 0, w, hh, Some(back), 0, 0, SRCCOPY);
            SelectObject(back, old_frame);
            let _ = DeleteObject(frame.into());
            let _ = DeleteDC(back);
        }
    }
    let _ = EndPaint(h, &ps);
}

/// Copy a region of the frozen shot as top-down BGRA, upscaled for small
/// regions (OCR reads small UI text much better at 2x).
unsafe fn crop(sel: &Selection, r: RECT) -> (Vec<u8>, u32, u32) {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let scale = if w.max(h) < 900 { 2 } else { 1 };
    let (ow, oh) = (w * scale, h * scale);
    let screen = GetDC(None);
    let src = CreateCompatibleDC(Some(screen));
    let old_src = SelectObject(src, HGDIOBJ(sel.shot as *mut _));
    let dst = CreateCompatibleDC(Some(screen));
    let out = CreateCompatibleBitmap(screen, ow, oh);
    let old_dst = SelectObject(dst, out.into());
    SetStretchBltMode(dst, HALFTONE);
    let _ = StretchBlt(dst, 0, 0, ow, oh, Some(src), r.left, r.top, w, h, SRCCOPY);
    SelectObject(dst, old_dst);
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ow,
            biHeight: -oh,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (ow * oh * 4) as usize];
    GetDIBits(dst, out, 0, oh as u32, Some(pixels.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
    for px in pixels.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let _ = DeleteObject(out.into());
    let _ = DeleteDC(dst);
    SelectObject(src, old_src);
    let _ = DeleteDC(src);
    ReleaseDC(None, screen);
    (pixels, ow as u32, oh as u32)
}

/// Close the overlay, free the bitmaps, and run OCR on `region` if any.
unsafe fn finish(region: Option<RECT>) {
    let sel = SELECTION.lock().ok().and_then(|mut s| s.take());
    let Some(sel) = sel else { return };
    let job = region
        .filter(|r| r.right - r.left > 4 && r.bottom - r.top > 4)
        .map(|r| crop(&sel, r));
    let _ = DeleteObject(HGDIOBJ(sel.shot as *mut _));
    let _ = DeleteObject(HGDIOBJ(sel.dim as *mut _));
    if let Some((pixels, w, h)) = job {
        std::thread::spawn(move || {
            let result = recognize(pixels, w, h);
            if let Ok(mut slot) = RESULT.lock() {
                *slot = Some(result);
            }
            let raw = NOTIFY.load(Ordering::Relaxed);
            if raw != 0 {
                let _ = PostMessageW(Some(HWND(raw as *mut _)), WM_APP_OCR, WPARAM(0), LPARAM(0));
            }
        });
    }
}

/// Run the Windows OCR engine on BGRA pixels. Uses the user's languages.
fn recognize(pixels: Vec<u8>, w: u32, h: u32) -> Result<String, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let fail = |e: windows::core::Error| format!("OCR failed: {}", e.message());
    let max = OcrEngine::MaxImageDimension().unwrap_or(2600);
    if w > max || h > max {
        return Err(format!("Area too large for OCR (max {max}px). Select a smaller area."));
    }
    let buffer = CryptographicBuffer::CreateFromByteArray(&pixels).map_err(fail)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32)
        .map_err(fail)?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|_| "No OCR language installed. Add one in Settings → Time & language → Language.".to_string())?;
    let op = engine.RecognizeAsync(&bitmap).map_err(fail)?;
    while op.Status().map_err(fail)? == windows_future::AsyncStatus::Started {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let result = op.GetResults().map_err(fail)?;
    let lines: Vec<String> = result
        .Lines()
        .map_err(fail)?
        .into_iter()
        .filter_map(|line| line.Text().ok().map(|t| t.to_string()))
        .collect();
    let text = lines.join("\n").trim().to_string();
    if text.is_empty() {
        Err("No text found in that area.".into())
    } else {
        Ok(text)
    }
}

fn point(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 & 0xFFFF) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    }
}

unsafe extern "system" fn proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(h);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            if let Ok(mut s) = SELECTION.lock() {
                if let Some(sel) = s.as_mut() {
                    sel.start = Some(point(lparam));
                    sel.current = point(lparam);
                }
            }
            let _ = SetCapture(h);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Ok(mut s) = SELECTION.lock() {
                if let Some(sel) = s.as_mut().filter(|sel| sel.start.is_some()) {
                    sel.current = point(lparam);
                    let _ = InvalidateRect(Some(h), None, false);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            let region = SELECTION
                .lock()
                .ok()
                .and_then(|s| s.as_ref().and_then(|sel| sel.start.map(|a| norm(a, point(lparam)))));
            let _ = DestroyWindow(h);
            finish(region);
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let _ = DestroyWindow(h);
            finish(None);
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => {
            let _ = DestroyWindow(h);
            finish(None);
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            // Focus stolen (e.g. Alt+Tab): cancel rather than linger.
            let _ = DestroyWindow(h);
            finish(None);
            LRESULT(0)
        }
        _ => DefWindowProcW(h, msg, wparam, lparam),
    }
}
