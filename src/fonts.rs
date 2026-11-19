//! Typefaces from Google Fonts: Red Hat Display for titles and numbers,
//! Instrument Serif for large display figures and accents, Inter for body
//! text.
//!
//! Nothing is installed into Windows. On first run the TTFs are fetched
//! from Google Fonts and kept in `%LOCALAPPDATA%\Notch\fonts`; they are
//! loaded privately for this process (a DirectWrite collection for the
//! notch, a private GDI registration for the Ask window). Offline on first
//! run means system fonts until the next launch.

use std::cell::RefCell;
use std::path::PathBuf;

use windows::core::Interface;
use windows::Win32::Graphics::DirectWrite::{IDWriteFactory, IDWriteFactory5, IDWriteFontCollection};

pub const DISPLAY: &str = "Red Hat Display";
pub const SERIF: &str = "Instrument Serif";
pub const BODY: &str = "Inter";

/// Google Fonts CSS2 request: the weights NOTCH draws with.
const CSS_PATH: &str = "/css2?family=Inter:wght@400;500;600;700\
                        &family=Red+Hat+Display:wght@500;600;700\
                        &family=Instrument+Serif&display=swap";

thread_local! {
    static COLLECTION: RefCell<Option<IDWriteFontCollection>> = const { RefCell::new(None) };
}

fn cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Notch")
        .join("fonts")
}

/// Font file URLs (`https://fonts.gstatic.com/...ttf`) from a CSS2 response.
pub fn font_urls(css: &str) -> Vec<String> {
    css.split("url(")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .map(|u| u.trim_matches(['\'', '"']).to_string())
        .filter(|u| u.starts_with("https://") && u.ends_with(".ttf"))
        .collect()
}

fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    let mut out = Vec::new();
    let status = crate::http::stream("GET", host, &format!("/{path}"), &[], None, |chunk| {
        out.extend_from_slice(chunk);
        out.len() < 8 * 1024 * 1024
    })?;
    ((200..300).contains(&status) && !out.is_empty()).then_some(out)
}

/// Cached font files, downloading them from Google Fonts when the cache
/// is empty. Returns the file paths that are ready.
pub fn ensure_cached() -> Vec<PathBuf> {
    let dir = cache_dir();
    let cached = |dir: &PathBuf| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "ttf"))
                    .collect()
            })
            .unwrap_or_default()
    };
    let have = cached(&dir);
    if !have.is_empty() {
        return have;
    }
    // A non-browser user agent gets plain TrueType links from the CSS API.
    let Some((200, css)) = crate::http::get("fonts.googleapis.com", CSS_PATH, &[]) else {
        crate::log("fonts: Google Fonts unreachable, using system fonts");
        return Vec::new();
    };
    let _ = std::fs::create_dir_all(&dir);
    for (i, url) in font_urls(&css).iter().enumerate() {
        if let Some(bytes) = fetch_bytes(url) {
            let _ = std::fs::write(dir.join(format!("font-{i:02}.ttf")), bytes);
        }
    }
    cached(&dir)
}

/// Build the DirectWrite collection for this (UI) thread.
pub fn init(factory: &IDWriteFactory) {
    if COLLECTION.with(|c| c.borrow().is_some()) {
        return;
    }
    let files = ensure_cached();
    if files.is_empty() {
        return;
    }
    match unsafe { build(factory, &files) } {
        Ok(collection) => COLLECTION.with(|c| *c.borrow_mut() = Some(collection)),
        Err(err) => crate::log(&format!("fonts: could not load ({err}), using system fonts")),
    }
    register_gdi(&files);
}

unsafe fn build(factory: &IDWriteFactory, files: &[PathBuf]) -> windows::core::Result<IDWriteFontCollection> {
    let factory5: IDWriteFactory5 = factory.cast()?;
    let builder = factory5.CreateFontSetBuilder()?;
    for path in files {
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let file = factory5.CreateFontFileReference(windows::core::PCWSTR(wide.as_ptr()), None)?;
        builder.AddFontFile(&file)?;
    }
    let set = builder.CreateFontSet()?;
    factory5.CreateFontCollectionFromFontSet(&set)?.cast()
}

/// The loaded collection when `family` is one of ours, else None (system).
pub fn collection_for(family: &str) -> Option<IDWriteFontCollection> {
    if ![DISPLAY, SERIF, BODY].contains(&family) {
        return None;
    }
    COLLECTION.with(|c| c.borrow().clone())
}

/// Register the files with GDI for this process only (FR_PRIVATE), so the
/// Ask window can use them too. Nothing is installed system-wide.
fn register_gdi(files: &[PathBuf]) {
    use windows::Win32::Graphics::Gdi::{AddFontResourceExW, FR_PRIVATE};
    for path in files {
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            AddFontResourceExW(windows::core::PCWSTR(wide.as_ptr()), FR_PRIVATE, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::font_urls;

    #[test]
    fn reads_css2_urls() {
        let css = "@font-face {\n  src: url(https://fonts.gstatic.com/s/inter/v20/abc.ttf) format('truetype');\n}\n\
                   @font-face {\n  src: url(https://fonts.gstatic.com/s/x/v1/def.woff2) format('woff2');\n}";
        assert_eq!(font_urls(css), vec!["https://fonts.gstatic.com/s/inter/v20/abc.ttf"]);
    }
}
