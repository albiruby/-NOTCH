//! AI Developer Cockpit: month-to-date spend and tokens per provider.
//!
//! Keys live in Windows Credential Manager (`notch key set <provider> <key>`).
//! A background thread refreshes every 15 minutes, or on demand when the hub
//! opens with stale data, and posts `WM_APP_AI` so the window redraws.
//!
//! - Claude: Usage and Cost Analytics API (Admin API key, `sk-ant-admin...`).
//! - OpenAI: organization Costs and Usage API (admin key).
//! - DeepSeek: account balance.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, Thread};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::ai_report::{self as report, Month, Provider, Stat};

pub const WM_APP_AI: u32 = WM_APP + 20;
pub const WM_APP_AI_REFRESH: u32 = WM_APP + 21;
const INTERVAL: Duration = Duration::from_secs(15 * 60);
const STALE: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub stats: Vec<(Provider, Stat)>,
    pub month: Month,
    /// Monthly budget in USD; 0 = none.
    pub budget: f64,
    pub updated: Option<Instant>,
    /// AI tools detected open right now (desktop apps + browser tabs).
    pub active_sessions: Vec<String>,
}

impl Snapshot {
    pub fn configured(&self) -> bool {
        self.stats.iter().any(|(_, s)| s.configured)
    }

    pub fn total(&self) -> f64 {
        self.stats.iter().filter_map(|(_, s)| s.cost).sum()
    }

    /// Daily spend summed across providers, day 1..=today.
    pub fn daily(&self) -> Vec<f64> {
        let mut out = vec![0.0; self.month.today as usize];
        for (_, stat) in &self.stats {
            for (i, v) in stat.daily.iter().enumerate() {
                if let Some(slot) = out.get_mut(i) {
                    *slot += v;
                }
            }
        }
        out
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn slot() -> &'static Mutex<Snapshot> {
    static SLOT: OnceLock<Mutex<Snapshot>> = OnceLock::new();
    SLOT.get_or_init(|| {
        Mutex::new(Snapshot {
            stats: Provider::ALL
                .iter()
                .map(|p| {
                    (
                        *p,
                        Stat {
                            configured: crate::secrets::load(p.key()).is_some(),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            month: Month::at(now_unix()),
            budget: load_budget(),
            updated: None,
            active_sessions: crate::sessions::active(),
        })
    })
}

static TARGET: AtomicIsize = AtomicIsize::new(0);
static WORKER: OnceLock<Thread> = OnceLock::new();

pub fn start(hwnd: HWND) {
    TARGET.store(hwnd.0 as isize, Ordering::Relaxed);
    let handle = thread::Builder::new()
        .name("notch-ai".into())
        .spawn(|| loop {
            let has_keys = Provider::ALL
                .iter()
                .any(|p| crate::secrets::load(p.key()).is_some());
            if has_keys {
                let next = fetch_all();
                if let Ok(mut guard) = slot().lock() {
                    *guard = next;
                }
            } else {
                // No keys: still refresh the lightweight "active now" line.
                let sessions = crate::sessions::active();
                if let Ok(mut guard) = slot().lock() {
                    guard.active_sessions = sessions;
                    guard.month = Month::at(now_unix());
                }
            }
            notify();
            thread::park_timeout(INTERVAL);
        });
    if let Ok(handle) = handle {
        let _ = WORKER.set(handle.thread().clone());
    }
}

/// Fetch now (e.g. after `notch key set`).
pub fn refresh() {
    if let Some(worker) = WORKER.get() {
        worker.unpark();
    }
}

/// Fetch if the last update is older than five minutes (hub opened).
pub fn refresh_if_stale() {
    let stale = slot()
        .lock()
        .map(|s| s.updated.is_none_or(|t| t.elapsed() > STALE))
        .unwrap_or(false);
    if stale {
        refresh();
    }
}

pub fn snapshot() -> Snapshot {
    slot()
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|e| e.into_inner().clone())
}

fn notify() {
    let raw = TARGET.load(Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(raw as *mut core::ffi::c_void)),
                WM_APP_AI,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

fn fetch_all() -> Snapshot {
    let month = Month::at(now_unix());
    let stats = Provider::ALL
        .iter()
        .map(|p| {
            let stat = match crate::secrets::load(p.key()) {
                None => Stat::default(),
                Some(key) => {
                    let mut stat = match p {
                        Provider::Anthropic => anthropic(&key, &month),
                        Provider::OpenAi => openai(&key, &month),
                        Provider::DeepSeek => deepseek(&key),
                    };
                    stat.configured = true;
                    stat
                }
            };
            (*p, stat)
        })
        .collect();
    Snapshot {
        stats,
        month,
        budget: load_budget(),
        updated: Some(Instant::now()),
        active_sessions: crate::sessions::active(),
    }
}

/// GET and keep the body only on 2xx; otherwise record why.
fn get(
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    error: &mut Option<String>,
) -> Option<String> {
    match crate::http::get(host, path, headers) {
        Some((status, body)) if (200..300).contains(&status) => Some(body),
        Some((status, _)) => {
            error.get_or_insert_with(|| report::status_error(status));
            None
        }
        None => {
            error.get_or_insert_with(|| "offline".into());
            None
        }
    }
}

fn anthropic(key: &str, month: &Month) -> Stat {
    let headers = [
        ("x-api-key", key),
        ("anthropic-version", "2026-10-21"),
        ("anthropic-beta", "analytics-usage-cost-2026-08-24"),
    ];
    let range = format!(
        "starting_at={}&ending_at={}&time_bucket_width=1d&order_by=time&order=asc&limit=1000",
        month.start_rfc3339(),
        month.end_rfc3339()
    );
    let mut stat = Stat::default();
    let host = "api.anthropic.com";
    if let Some(body) = get(
        host,
        &format!("/v1/organizations/analytics/cost?{range}"),
        &headers,
        &mut stat.error,
    ) {
        stat.daily = report::anthropic_cost(&body, month).unwrap_or_default();
        stat.cost = Some(stat.daily.iter().sum());
    }
    if let Some(body) = get(
        host,
        &format!("/v1/organizations/analytics/usage?{range}"),
        &headers,
        &mut stat.error,
    ) {
        stat.tokens = report::anthropic_tokens(&body);
    }
    stat
}

fn openai(key: &str, month: &Month) -> Stat {
    let bearer = format!("Bearer {key}");
    let headers = [("Authorization", bearer.as_str())];
    let range = format!("start_time={}&bucket_width=1d&limit=31", month.start_unix);
    let mut stat = Stat::default();
    let host = "api.openai.com";
    if let Some(body) = get(
        host,
        &format!("/v1/organization/costs?{range}"),
        &headers,
        &mut stat.error,
    ) {
        stat.daily = report::openai_cost(&body, month).unwrap_or_default();
        stat.cost = Some(stat.daily.iter().sum());
    }
    if let Some(body) = get(
        host,
        &format!("/v1/organization/usage/completions?{range}"),
        &headers,
        &mut stat.error,
    ) {
        stat.tokens = report::openai_tokens(&body);
    }
    stat
}

fn deepseek(key: &str) -> Stat {
    let bearer = format!("Bearer {key}");
    let mut stat = Stat::default();
    if let Some(body) = get(
        "api.deepseek.com",
        "/user/balance",
        &[("Authorization", bearer.as_str())],
        &mut stat.error,
    ) {
        stat.balance = report::deepseek_balance(&body);
    }
    stat
}

// ---- budget (plain file next to settings.json; not a secret) ----

fn budget_path() -> std::path::PathBuf {
    crate::settings::Settings::dir().join("ai.json")
}

fn load_budget() -> f64 {
    std::fs::read_to_string(budget_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("budget_usd")?.as_f64())
        .unwrap_or(0.0)
}

fn save_budget(usd: f64) -> bool {
    let _ = std::fs::create_dir_all(crate::settings::Settings::dir());
    std::fs::write(budget_path(), format!("{{\"budget_usd\": {usd}}}\n")).is_ok()
}

// ---- CLI ----

/// Handles `notch key ...` and `notch ai ...`. Returns the exit code, or
/// None when the arguments are not a cockpit command.
pub fn cli(args: &[String]) -> Option<i32> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let code = match words.as_slice() {
        ["key", "set", provider, key] => match Provider::parse(provider) {
            Some(p) if crate::secrets::store(p.key(), key) => {
                println!("Saved {} key to Windows Credential Manager.", p.label());
                ping_running();
                0
            }
            Some(_) => {
                eprintln!("Could not write to Credential Manager.");
                1
            }
            None => usage(),
        },
        ["key", "clear", provider] => match Provider::parse(provider) {
            Some(p) => {
                crate::secrets::delete(p.key());
                println!("Removed {} key.", p.label());
                ping_running();
                0
            }
            None => usage(),
        },
        ["ai", "budget", usd] => match usd.trim_start_matches('$').parse::<f64>() {
            Ok(value) if value >= 0.0 && save_budget(value) => {
                println!("Monthly AI budget set to ${value:.2}.");
                ping_running();
                0
            }
            _ => usage(),
        },
        ["ai", "refresh"] => {
            ping_running();
            0
        }
        ["key", ..] | ["ai", ..] => usage(),
        _ => return None,
    };
    Some(code)
}

fn usage() -> i32 {
    eprintln!(
        "usage:\n  notch key set <claude|openai|deepseek> <key>\n  notch key clear <provider>\n  notch ai budget <usd>\n  notch ai refresh"
    );
    2
}

fn ping_running() {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe {
        if let Ok(target) = FindWindowW(w!("NotchSurface"), None) {
            let _ = PostMessageW(Some(target), WM_APP_AI_REFRESH, WPARAM(0), LPARAM(0));
        }
    }
}
