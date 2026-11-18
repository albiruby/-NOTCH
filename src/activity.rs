//! Live Activities: other programs push progress into the island.
//!
//! The same `notch.exe` doubles as the client:
//!
//! ```text
//! notch push "Build" --progress 40 --detail "cargo build --release"
//! notch push "Build" --done
//! notch push "Deploy" --fail --detail "exit code 1"
//! notch clear [title]
//! ```
//!
//! The client finds the running NOTCH window and hands it the request over
//! `WM_COPYDATA` as JSON, then exits. Activities are keyed by title, so
//! repeated pushes update one row. Finished ones fade after a few seconds.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// `COPYDATASTRUCT::dwData` tag so foreign WM_COPYDATA is ignored.
pub const COPYDATA_TAG: usize = 0x4E4F_5443; // "NOTC"
pub const MAX: usize = 3;
const FINISHED_TTL: Duration = Duration::from_secs(6);
const STALE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Push {
        title: String,
        detail: Option<String>,
        /// 0..=1, or None for an indeterminate activity.
        progress: Option<f32>,
        state: Status,
    },
    Clear {
        title: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub title: String,
    pub detail: Option<String>,
    pub progress: Option<f32>,
    pub status: Status,
    updated: Instant,
}

#[derive(Default)]
pub struct Store {
    pub items: Vec<Activity>,
}

impl Store {
    pub fn apply(&mut self, request: Request) {
        match request {
            Request::Push {
                title,
                detail,
                progress,
                state,
            } => {
                let progress = progress.map(|p| p.clamp(0.0, 1.0));
                if let Some(item) = self.items.iter_mut().find(|a| a.title == title) {
                    if detail.is_some() {
                        item.detail = detail;
                    }
                    if progress.is_some() || state != Status::Running {
                        item.progress = if state == Status::Done {
                            Some(1.0)
                        } else {
                            progress.or(item.progress)
                        };
                    }
                    item.status = state;
                    item.updated = Instant::now();
                } else {
                    self.items.insert(
                        0,
                        Activity {
                            title,
                            detail,
                            progress: if state == Status::Done {
                                Some(1.0)
                            } else {
                                progress
                            },
                            status: state,
                            updated: Instant::now(),
                        },
                    );
                    self.items.truncate(MAX);
                }
                // Most recently touched first.
                self.items.sort_by_key(|a| std::cmp::Reverse(a.updated));
            }
            Request::Clear { title } => match title {
                Some(title) => self.items.retain(|a| a.title != title),
                None => self.items.clear(),
            },
        }
    }

    /// Drops finished and stale activities. Returns true if anything changed.
    pub fn expire(&mut self) -> bool {
        let before = self.items.len();
        self.items.retain(|a| {
            let age = a.updated.elapsed();
            match a.status {
                Status::Running => age < STALE_TTL,
                _ => age < FINISHED_TTL,
            }
        });
        before != self.items.len()
    }
}

/// Parses `push`/`clear` command lines. `args` excludes the program name.
pub fn parse(args: &[String]) -> Option<Request> {
    let (command, rest) = args.split_first()?;
    match command.as_str() {
        "push" => {
            let mut title = None;
            let mut detail = None;
            let mut progress = None;
            let mut state = Status::Running;
            let mut iter = rest.iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--detail" | "-d" => detail = iter.next().cloned(),
                    "--progress" | "-p" => {
                        progress = iter
                            .next()
                            .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
                            .map(|v| if v > 1.0 { v / 100.0 } else { v });
                    }
                    "--done" => state = Status::Done,
                    "--fail" | "--failed" => state = Status::Failed,
                    other if title.is_none() && !other.starts_with("--") => {
                        title = Some(other.to_string())
                    }
                    _ => {}
                }
            }
            Some(Request::Push {
                title: title?,
                detail,
                progress,
                state,
            })
        }
        "clear" => Some(Request::Clear {
            title: rest.first().cloned(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_push() {
        let req = parse(&args(&[
            "push",
            "Build",
            "--progress",
            "40",
            "--detail",
            "cargo",
        ]))
        .unwrap();
        assert_eq!(
            req,
            Request::Push {
                title: "Build".into(),
                detail: Some("cargo".into()),
                progress: Some(0.4),
                state: Status::Running
            }
        );
        let req = parse(&args(&["push", "Build", "--done"])).unwrap();
        assert!(matches!(
            req,
            Request::Push {
                state: Status::Done,
                ..
            }
        ));
        assert!(parse(&args(&["push"])).is_none());
        assert!(parse(&args(&["--expanded"])).is_none());
    }

    #[test]
    fn updates_by_title() {
        let mut store = Store::default();
        store.apply(parse(&args(&["push", "A", "-p", "10"])).unwrap());
        store.apply(parse(&args(&["push", "B"])).unwrap());
        store.apply(parse(&args(&["push", "A", "-p", "0.5"])).unwrap());
        assert_eq!(store.items.len(), 2);
        assert_eq!(store.items[0].title, "A");
        assert_eq!(store.items[0].progress, Some(0.5));
        store.apply(parse(&args(&["push", "A", "--done"])).unwrap());
        assert_eq!(store.items[0].progress, Some(1.0));
        store.apply(parse(&args(&["clear"])).unwrap());
        assert!(store.items.is_empty());
    }
}
