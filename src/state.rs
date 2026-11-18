//! Geometry and animation state for the notch surface.
//!
//! All values are in device-independent pixels (DIP). The renderer scales them
//! by the monitor DPI before drawing.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Compact,
    Expanded,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceMode {
    /// Classic notch: attached to the top edge, square top corners, rounded bottom.
    Notch,
    /// Dynamic island: floating pill with a gap from the top edge.
    Island,
    /// iPhone X style notch: pure black, attached to the top edge, with
    /// concave "shoulders" where it meets the screen edge.
    #[serde(rename = "notch_x")]
    NotchX,
    /// iPhone 14 Pro style Dynamic Island: a compact pure-black pill with a
    /// camera lens on its right end. Floats and can be dragged like Island.
    #[serde(rename = "island_pro")]
    IslandPro,
}

impl SurfaceMode {
    /// Attached to the top edge (square top, rounded bottom).
    pub fn attached(self) -> bool {
        matches!(self, SurfaceMode::Notch | SurfaceMode::NotchX)
    }
    /// Pure black hardware look instead of glass.
    pub fn solid(self) -> bool {
        matches!(self, SurfaceMode::NotchX | SurfaceMode::IslandPro)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Vertical panel with paged content.
    Panel,
    /// One wide bar with media, metrics, and clock.
    Wide,
    /// Two floating cards: quick controls and file shelf.
    Cards,
    /// Click the pill to open every widget at once (launcher, clipboard,
    /// media + system, focus, quick controls, file shelf).
    #[default]
    Hub,
}

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub surface_w: f32,
    pub surface_h: f32,
    pub compact_w: f32,
    pub compact_h: f32,
    pub compact_r: f32,
    pub expanded_w: f32,
    pub expanded_h: f32,
    pub expanded_r: f32,
    pub top_gap: f32,
    pub square_top: bool,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::from_settings(&crate::settings::Settings::default())
    }
}

impl Metrics {
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        Self::for_layout(settings, settings.layout)
    }

    pub fn for_layout(settings: &crate::settings::Settings, layout: Layout) -> Self {
        let compact_h = settings.compact_height.max(20.0);
        let mode = settings.mode;
        let square_top = mode.attached();
        let (surface_w, surface_h, expanded_w, expanded_h, expanded_r) =
            if layout == Layout::Wide {
                // One wide bar: media, transport, metrics, and clock in a row.
                (996.0, 188.0, 900.0, 140.0, 28.0)
            } else if layout == Layout::Cards {
                (424.0, 568.0, 424.0, 568.0, 0.0)
            } else if layout == Layout::Hub {
                // The pill grows into a top bar; cards float beneath it.
                (hub::w(), hub::h(), 620.0, 52.0, 26.0)
            } else {
                // The expanded panel has a fixed content layout, so it never
                // shrinks below the height that layout needs.
                let expanded_h = settings.expanded_height.max(300.0);
                (
                    (settings.expanded_width + 96.0).max(420.0),
                    expanded_h + 48.0,
                    settings.expanded_width.max(300.0),
                    expanded_h,
                    crate::theme::radius::XL,
                )
            };
        Self {
            surface_w,
            surface_h,
            // The Pro island is the short, tall pill of recent iPhones; the
            // iPhone X notch is a little wider and shallower.
            compact_w: match mode {
                SurfaceMode::IslandPro => 168.0,
                SurfaceMode::NotchX => settings.compact_width.clamp(120.0, 220.0),
                _ => settings.compact_width.max(120.0),
            },
            compact_h: if mode == SurfaceMode::IslandPro { compact_h.max(36.0) } else { compact_h },
            compact_r: if mode == SurfaceMode::IslandPro { compact_h.max(36.0) * 0.5 } else { compact_h * 0.5 },
            expanded_w,
            expanded_h,
            expanded_r,
            top_gap: if square_top { 0.0 } else { 10.0 },
            square_top,
        }
    }
}

/// A rounded rectangle in window-relative DIP coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
}

impl Shape {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Point-in-shape test, including the rounded corners.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        if px < self.x || px > self.right() || py < self.y || py > self.bottom() {
            return false;
        }

        let r = self.r.max(0.0);
        if r == 0.0 {
            return true;
        }

        let cx = px.clamp(self.x + r, self.right() - r);
        let cy = py.clamp(self.y + r, self.bottom() - r);
        let dx = px - cx;
        let dy = py - cy;
        dx * dx + dy * dy <= r * r
    }
}

pub struct AppState {
    pub phase: Phase,
    /// 0.0 = compact, 1.0 = expanded.
    pub t: f32,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Compact,
            t: 0.0,
        }
    }

    pub fn shape(&self, m: &Metrics) -> Shape {
        let t = self.t.clamp(0.0, 1.0);

        // In notch mode the shape is pushed above the surface by its radius so
        // the top edge is clipped square and only the bottom corners are round.
        let (compact_y, compact_h) = if m.square_top {
            (-m.compact_r, m.compact_h + m.compact_r)
        } else {
            (m.top_gap, m.compact_h)
        };
        let (expanded_y, expanded_h) = if m.square_top {
            (-m.expanded_r, m.expanded_h + m.expanded_r)
        } else {
            (m.top_gap, m.expanded_h)
        };

        let w = lerp(m.compact_w, m.expanded_w, t);
        let h = lerp(compact_h, expanded_h, t);
        let y = lerp(compact_y, expanded_y, t);
        let r = lerp(m.compact_r, m.expanded_r, t);
        let x = (m.surface_w - w) * 0.5;

        Shape { x, y, w, h, r }
    }

    /// Visible bounds used for hit testing (the part inside the surface).
    pub fn hit_shape(&self, m: &Metrics) -> Shape {
        let mut s = self.shape(m);
        if s.y < 0.0 {
            s.h += s.y;
            s.y = 0.0;
        }
        s
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Monotonic id used to signal "this payload changed" across threads without
/// hashing the payload itself.
pub fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

pub fn quick_card() -> Rect {
    Rect {
        x: 12.0,
        y: 12.0,
        w: 400.0,
        h: 330.0,
    }
}

pub fn shelf_card() -> Rect {
    Rect {
        x: 12.0,
        y: 358.0,
        w: 400.0,
        h: 198.0,
    }
}

pub struct CardHits {
    pub volume: Rect,
    pub brightness: Rect,
    pub focus: Rect,
    pub battery: Rect,
    pub record: Rect,
    pub theme: Rect,
    pub clear: Rect,
    pub rows: [Rect; 5],
}

pub fn card_hits() -> CardHits {
    let row = |index: usize| Rect {
        x: 32.0,
        y: 398.0 + index as f32 * 28.0,
        w: 360.0,
        h: 26.0,
    };
    CardHits {
        volume: Rect {
            x: 32.0,
            y: 82.0,
            w: 360.0,
            h: 38.0,
        },
        brightness: Rect {
            x: 32.0,
            y: 128.0,
            w: 360.0,
            h: 38.0,
        },
        focus: Rect {
            x: 32.0,
            y: 192.0,
            w: 172.0,
            h: 30.0,
        },
        battery: Rect {
            x: 216.0,
            y: 192.0,
            w: 172.0,
            h: 30.0,
        },
        record: Rect {
            x: 32.0,
            y: 230.0,
            w: 172.0,
            h: 30.0,
        },
        theme: Rect {
            x: 216.0,
            y: 230.0,
            w: 172.0,
            h: 30.0,
        },
        clear: Rect {
            x: 340.0,
            y: 366.0,
            w: 50.0,
            h: 16.0,
        },
        rows: [row(0), row(1), row(2), row(3), row(4)],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    Volume,
    Caps,
    Water,
    Copied,
}

/// Transient compact-pill content shown after a hardware key event.
#[derive(Clone, Copy, Debug)]
pub struct Overlay {
    pub kind: OverlayKind,
    pub value: f32,
    pub muted: bool,
    pub on: bool,
    pub goal: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[allow(dead_code)]
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

pub struct Tabs {
    pub overview: Rect,
    pub controls: Rect,
    pub focus: Rect,
    pub tasks: Rect,
    pub shelf: Rect,
    pub clip: Rect,
    pub apps: Rect,
}

pub fn tabs(shape: &Shape) -> Tabs {
    let top = shape.y.max(0.0);
    let cx = shape.x + shape.w * 0.5;
    let y = top + 262.0;
    let (w, gap) = (60.0, 4.0);
    let start = cx - (w * 7.0 + gap * 6.0) * 0.5;
    let make = |index: usize| Rect {
        x: start + index as f32 * (w + gap),
        y,
        w,
        h: 22.0,
    };
    Tabs {
        overview: make(0),
        controls: make(1),
        focus: make(2),
        tasks: make(3),
        shelf: make(4),
        clip: make(5),
        apps: make(6),
    }
}

pub fn task_row(shape: &Shape, index: usize) -> Rect {
    let top = shape.y.max(0.0);
    Rect {
        x: shape.x + 24.0,
        y: top + 112.0 + index as f32 * 22.0,
        w: shape.w - 48.0,
        h: 20.0,
    }
}

pub fn task_add(shape: &Shape) -> Rect {
    let top = shape.y.max(0.0);
    Rect {
        x: shape.x + 24.0,
        y: top + 202.0,
        w: shape.w - 48.0,
        h: 20.0,
    }
}

pub fn water_dot(shape: &Shape, index: usize) -> Rect {
    let top = shape.y.max(0.0);
    let count = 8usize;
    let step = 16.0;
    let start = shape.x + shape.w * 0.5 - (count as f32 - 1.0) * step * 0.5;
    Rect {
        x: start + index as f32 * step - 6.0,
        y: top + 228.0,
        w: 12.0,
        h: 16.0,
    }
}

pub fn clip_row(shape: &Shape, index: usize) -> Rect {
    let top = shape.y.max(0.0);
    Rect {
        x: shape.x + 24.0,
        y: top + 114.0 + index as f32 * 24.0,
        w: shape.w - 48.0,
        h: 22.0,
    }
}

pub fn clip_clear(shape: &Shape) -> Rect {
    let top = shape.y.max(0.0);
    Rect {
        x: shape.right() - 84.0,
        y: top + 94.0,
        w: 60.0,
        h: 16.0,
    }
}

pub fn focus_toggle(shape: &Shape) -> Rect {
    let top = shape.y.max(0.0);
    Rect {
        x: shape.x + 60.0,
        y: top + 70.0,
        w: shape.w - 120.0,
        h: 140.0,
    }
}

pub fn focus_reset(shape: &Shape) -> Rect {
    let top = shape.y.max(0.0);
    let cx = shape.x + shape.w * 0.5;
    Rect {
        x: cx - 50.0,
        y: top + 214.0,
        w: 100.0,
        h: 24.0,
    }
}

pub fn progress_bar(shape: &Shape) -> Rect {
    let top = shape.y.max(0.0);
    Rect {
        x: shape.x + 24.0,
        y: top + 88.0,
        w: shape.w - 48.0,
        h: 18.0,
    }
}

pub struct Transport {
    pub prev: Rect,
    pub play: Rect,
    pub next: Rect,
}

pub struct Controls {
    pub volume: Rect,
    pub brightness: Rect,
}

/// Quick controls share one layout between the renderer and hit testing.
///
/// These rects live on the Controls page only. Their y positions must match
/// the labels and bars drawn at `ctrl_y` in the renderer.
pub fn controls(shape: &Shape) -> Controls {
    let top = shape.y.max(0.0);
    let pad = 24.0;
    Controls {
        volume: Rect {
            x: shape.x + pad,
            y: top + 94.0,
            w: shape.w - pad * 2.0,
            h: 34.0,
        },
        brightness: Rect {
            x: shape.x + pad,
            y: top + 128.0,
            w: shape.w - pad * 2.0,
            h: 34.0,
        },
    }
}

pub struct WideLayout {
    pub progress: Rect,
    pub prev: Rect,
    pub play: Rect,
    pub next: Rect,
}

/// Hit rects for the wide bar. Coordinates mirror the renderer's bar layout.
pub fn wide_layout(shape: &Shape) -> WideLayout {
    let top = shape.y.max(0.0);
    let size = 32.0;
    let center_y = top + 50.0;
    let make = |center_x: f32| Rect {
        x: shape.x + center_x - size * 0.5,
        y: center_y - size * 0.5,
        w: size,
        h: size,
    };
    WideLayout {
        progress: Rect {
            x: shape.x + 84.0,
            y: top + 70.0,
            w: 276.0,
            h: 18.0,
        },
        prev: make(440.0),
        play: make(484.0),
        next: make(528.0),
    }
}

/// Transport buttons share one layout between the renderer and hit testing.
pub fn transport(shape: &Shape) -> Transport {
    let top = shape.y.max(0.0);
    let cx = shape.x + shape.w * 0.5;
    let cy = top + 136.0;
    let size = 32.0;
    let gap = 52.0;
    let make = |center: f32| Rect {
        x: center - size * 0.5,
        y: cy - size * 0.5,
        w: size,
        h: size,
    };
    Transport {
        prev: make(cx - gap),
        play: make(cx),
        next: make(cx + gap),
    }
}

/// Critically damped spring, stepped at whatever rate the frame loop runs at.
/// No fixed duration, so a delayed frame never stretches the animation.
pub struct Spring {
    pub value: f32,
    target: f32,
    velocity: f32,
}

impl Spring {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            velocity: 0.0,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Returns true while the spring is still moving.
    pub fn step(&mut self, dt: f32) -> bool {
        self.step_with(dt, crate::theme::motion::SPRING_OMEGA, 1.0)
    }

    /// Spring step with explicit stiffness and damping ratio. A ratio below 1
    /// overshoots slightly, which is what makes the island snap feel springy.
    pub fn step_with(&mut self, dt: f32, omega: f32, zeta: f32) -> bool {
        let x = self.value - self.target;
        let accel = -omega * omega * x - 2.0 * zeta * omega * self.velocity;
        self.velocity += accel * dt;
        self.value += self.velocity * dt;

        if x.abs() < 0.003 && self.velocity.abs() < 0.05 {
            self.value = self.target;
            self.velocity = 0.0;
            false
        } else {
            true
        }
    }
}

/// Geometry of the Hub overlay, in DIPs relative to the hub canvas.
///
/// The canvas fills the monitor's work area, so cards sit against the
/// screen edges (left column, right column, centre under the top bar, dock
/// at the bottom) and the middle of the desktop stays clear.
pub mod hub {
    use super::Rect;
    use std::sync::atomic::{AtomicU32, Ordering};

    static CANVAS_W: AtomicU32 = AtomicU32::new(0x449B0000); // 1240.0
    static CANVAS_H: AtomicU32 = AtomicU32::new(0x443E0000); // 760.0

    /// Smallest canvas the layout is designed for; smaller screens scroll
    /// the bottom row off-screen instead of overlapping cards.
    pub const MIN_W: f32 = 1240.0;
    pub const MIN_H: f32 = 760.0;

    /// Outer margin from the screen edge and gap between stacked cards.
    pub const MARGIN: f32 = 28.0;
    pub const GAP: f32 = 24.0;
    /// First card row starts below the top bar.
    pub const TOP: f32 = 92.0;

    const SIDE_W: f32 = 340.0;
    const RIGHT_W: f32 = 400.0;
    const MEDIA_W: f32 = 460.0;
    const MEDIA_H: f32 = 190.0;
    const CLIP_H: f32 = 232.0;
    const FOCUS_H: f32 = 150.0;
    const QUICK_H: f32 = 270.0;
    const SHELF_H: f32 = 160.0;
    const DOCK_ITEM: f32 = 56.0;
    const DOCK_H: f32 = 68.0;

    pub fn set_canvas(w: f32, h: f32) {
        CANVAS_W.store(w.max(MIN_W).to_bits(), Ordering::Relaxed);
        CANVAS_H.store(h.max(MIN_H).to_bits(), Ordering::Relaxed);
    }
    pub fn w() -> f32 {
        f32::from_bits(CANVAS_W.load(Ordering::Relaxed))
    }
    pub fn h() -> f32 {
        f32::from_bits(CANVAS_H.load(Ordering::Relaxed))
    }

    const fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    // Left column: launcher on top, clipboard pinned to the bottom.
    pub fn clip() -> Rect {
        r(MARGIN, h() - MARGIN - CLIP_H, SIDE_W, CLIP_H)
    }
    pub fn launcher() -> Rect {
        let bottom = clip().y - GAP;
        r(MARGIN, TOP, SIDE_W, (bottom - TOP).clamp(260.0, 440.0))
    }

    // Centre: media + system under the top bar.
    pub fn media() -> Rect {
        r((w() - MEDIA_W) * 0.5, TOP, MEDIA_W, MEDIA_H)
    }

    // Right column: focus and quick controls on top, shelf at the bottom.
    fn right_x() -> f32 {
        w() - MARGIN - RIGHT_W
    }
    pub fn focus() -> Rect {
        r(right_x(), TOP, RIGHT_W, FOCUS_H)
    }
    pub fn quick() -> Rect {
        r(right_x(), TOP + FOCUS_H + GAP, RIGHT_W, QUICK_H)
    }
    pub fn shelf() -> Rect {
        let below_quick = quick().y + QUICK_H + GAP;
        r(right_x(), (h() - MARGIN - SHELF_H).max(below_quick), RIGHT_W, SHELF_H)
    }

    // Bottom centre: a dock with one tile per pinned app.
    pub const DOCK_MAX: usize = 8;
    pub fn dock(count: usize) -> Option<Rect> {
        if count == 0 {
            return None;
        }
        let n = count.min(DOCK_MAX) as f32;
        let width = n * DOCK_ITEM + 16.0;
        Some(r((w() - width) * 0.5, h() - MARGIN - DOCK_H, width, DOCK_H))
    }
    pub fn dock_item(count: usize, index: usize) -> Rect {
        let dock = dock(count).unwrap_or(r(0.0, 0.0, 0.0, 0.0));
        r(dock.x + 8.0 + index as f32 * DOCK_ITEM + 4.0, dock.y + 10.0, 48.0, 48.0)
    }

    pub fn cards() -> [Rect; 6] {
        [launcher(), clip(), media(), focus(), quick(), shelf()]
    }

    /// Offsets that map the Cards-layout quick/shelf geometry into the hub.
    pub fn quick_offset() -> (f32, f32) {
        let q = quick();
        (q.x - 12.0, q.y - 12.0)
    }
    pub fn shelf_offset() -> (f32, f32) {
        let s = shelf();
        (s.x - 12.0, s.y - 358.0)
    }

    pub const APP_ROWS: usize = 6;
    pub fn app_rows() -> usize {
        let l = launcher();
        (((l.h - 84.0) / 56.0).floor().max(0.0) as usize).min(APP_ROWS)
    }
    pub fn app_row(index: usize) -> Rect {
        let l = launcher();
        r(l.x + 12.0, l.y + 72.0 + index as f32 * 56.0, l.w - 24.0, 52.0)
    }
    pub const CLIP_ROWS: usize = 5;
    pub fn clip_row(index: usize) -> Rect {
        let c = clip();
        r(c.x + 12.0, c.y + 44.0 + index as f32 * 36.0, c.w - 24.0, 34.0)
    }
    pub fn clip_clear() -> Rect {
        let c = clip();
        r(c.x + c.w - 84.0, c.y + 14.0, 72.0, 22.0)
    }

    pub fn prev() -> Rect {
        let m = media();
        r(m.x + 124.0, m.y + 66.0, 36.0, 36.0)
    }
    pub fn play() -> Rect {
        let m = media();
        r(m.x + 168.0, m.y + 66.0, 36.0, 36.0)
    }
    pub fn next() -> Rect {
        let m = media();
        r(m.x + 212.0, m.y + 66.0, 36.0, 36.0)
    }
    pub fn progress() -> Rect {
        let m = media();
        r(m.x + 124.0, m.y + 116.0, 168.0, 18.0)
    }

    pub fn focus_toggle() -> Rect {
        let f = focus();
        r(f.x + 220.0, f.y + 92.0, 80.0, 34.0)
    }
    pub fn focus_reset() -> Rect {
        let f = focus();
        r(f.x + 308.0, f.y + 92.0, 76.0, 34.0)
    }
}
