//! Detect AI tools the user already has open, to show "active now" in the
//! cockpit. Reads only what the OS shows anyone: running process names and
//! visible window titles. Never touches another app's account, cookies,
//! tokens or conversation content.

use std::collections::BTreeSet;

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, MAX_PATH};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
};

/// A known AI tool and how to recognise it.
struct Known {
    label: &'static str,
    /// Lowercase executable names (desktop apps).
    exes: &'static [&'static str],
    /// Lowercase substrings that appear in a browser tab's window title.
    title_marks: &'static [&'static str],
}

const KNOWN: &[Known] = &[
    Known {
        label: "Claude",
        exes: &["claude.exe"],
        title_marks: &["claude"],
    },
    Known {
        label: "ChatGPT",
        exes: &["chatgpt.exe"],
        title_marks: &["chatgpt"],
    },
    Known {
        label: "Gemini",
        exes: &["gemini.exe"],
        title_marks: &["gemini"],
    },
    Known {
        label: "DeepSeek",
        exes: &["deepseek.exe"],
        title_marks: &["deepseek"],
    },
    Known {
        label: "Copilot",
        exes: &["copilot.exe"],
        title_marks: &["copilot"],
    },
    Known {
        label: "Cursor",
        exes: &["cursor.exe"],
        title_marks: &[],
    },
    Known {
        label: "Perplexity",
        exes: &["perplexity.exe"],
        title_marks: &["perplexity"],
    },
];

/// Browser window titles are only matched when the title also looks like a
/// browser (so a file called "claude-notes.txt" in an editor doesn't count).
const BROWSER_MARKS: &[&str] = &[
    " - google chrome",
    " - microsoft\u{200b} edge",
    " - microsoft edge",
    " \u{2014} mozilla firefox",
    " - mozilla firefox",
    " - brave",
    " and \u{2014} brave",
    " - opera",
    " - arc",
];

/// Decide which known tools are present, given the lowercase process names
/// and the lowercase visible window titles. Pure, so it is unit-tested.
pub fn detect(processes: &BTreeSet<String>, titles: &[String]) -> Vec<String> {
    let mut active: Vec<String> = Vec::new();
    for known in KNOWN {
        let by_exe = known.exes.iter().any(|e| processes.contains(*e));
        let by_tab = !known.title_marks.is_empty()
            && titles.iter().any(|title| {
                let is_browser = BROWSER_MARKS.iter().any(|m| title.contains(m));
                is_browser && known.title_marks.iter().any(|m| title.contains(*m))
            });
        if by_exe || by_tab {
            let suffix = if by_exe { "" } else { " (web)" };
            active.push(format!("{}{}", known.label, suffix));
        }
    }
    active
}

/// Snapshot of running process names, lowercased.
fn processes() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return names;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(MAX_PATH as usize);
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
                if !name.is_empty() {
                    names.insert(name);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }
    names
}

fn titles() -> Vec<String> {
    let mut out: Box<Vec<String>> = Box::default();
    unsafe {
        let ptr = &mut *out as *mut Vec<String> as isize;
        let _ = EnumWindows(Some(collect), LPARAM(ptr));
    }
    *out
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if IsWindowVisible(hwnd).as_bool() {
        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; len as usize + 1];
            let copied = GetWindowTextW(hwnd, &mut buf);
            if copied > 0 {
                let title = String::from_utf16_lossy(&buf[..copied as usize]).to_lowercase();
                let list = &mut *(lparam.0 as *mut Vec<String>);
                list.push(title);
            }
        }
    }
    BOOL(1)
}

/// Tools that look active right now, e.g. ["Claude", "ChatGPT (web)"].
pub fn active() -> Vec<String> {
    detect(&processes(), &titles())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn desktop_and_web() {
        let procs = set(&["claude.exe", "explorer.exe"]);
        let titles = vec![
            "chatgpt - google chrome".to_string(),
            "inbox (3) - google chrome".to_string(),
        ];
        let got = detect(&procs, &titles);
        assert_eq!(got, vec!["Claude".to_string(), "ChatGPT (web)".to_string()]);
    }

    #[test]
    fn title_without_browser_is_ignored() {
        // An editor showing a file named "claude" must not count as a session.
        let procs = set(&["notepad.exe"]);
        let titles = vec!["claude-notes.txt - notepad".to_string()];
        assert!(detect(&procs, &titles).is_empty());
    }

    #[test]
    fn cursor_desktop_only() {
        assert_eq!(
            detect(&set(&["cursor.exe"]), &[]),
            vec!["Cursor".to_string()]
        );
        // Cursor has no title mark, so a browser tab mentioning it is ignored.
        assert!(detect(&set(&[]), &["cursor tips - brave".to_string()]).is_empty());
    }
}
