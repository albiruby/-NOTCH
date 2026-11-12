#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod apps;
mod audio;
mod brightness;
mod clipboard;
mod lyrics;
mod media;
mod netstatus;
mod renderer;
mod settings;
mod shelf;
mod state;
mod sysmon;
mod tasks;
mod theme;
mod tray;
mod window;

use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Instant;

use windows::core::{w, PCWSTR, Result};
use windows::Win32::Foundation::{
    HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WAIT_EVENT, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, ScreenToClient, PAINTSTRUCT};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{
    AttachThreadInput, CancelWaitableTimer, CreateMutexW, CreateWaitableTimerExW, GetCurrentThreadId,
    SetWaitableTimer, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, INFINITE, TIMER_ALL_ACCESS,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    VK_CAPITAL,
};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, DragQueryPoint, ShellExecuteW, HDROP,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use audio::Audio;
use brightness::Brightness;
use renderer::{DrawParams, Renderer};
use settings::Settings;
use state::{AppState, Layout, Metrics, Overlay, OverlayKind, Phase, Spring, SurfaceMode};
use sysmon::SysMon;
use tray::Tray;

const TIMER_TICK: usize = 2;
const TIMER_TRACE: usize = 3;
const TIMER_OVERLAY: usize = 4;
const TIMER_WATER: usize = 5;
const WM_APP_REFRESH: u32 = WM_APP + 1;
const WM_APP_REDRAW: u32 = WM_APP + 2;
const WM_APP_CAPS: u32 = WM_APP + 7;
const HOVER_MARGIN: f32 = 14.0;
const FRAME_MS: i64 = 16;
const OVERLAY_MS: u32 = 1500;

static NOTCH_HWND: AtomicIsize = AtomicIsize::new(0);
static MENU_OWNER: AtomicIsize = AtomicIsize::new(0);
static ANIM_TIMER: AtomicIsize = AtomicIsize::new(0);

pub fn log(msg: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("notch.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{msg}");
    }
}

struct App {
    hwnd: HWND,
    renderer: Renderer,
    state: AppState,
    metrics: Metrics,
    settings: Settings,
    sysmon: SysMon,
    scale: f32,
    spring: Spring,
    animating: bool,
    last_frame: Instant,
    trace: bool,
    trace_start: Instant,
    trace_stage: u8,
    tracking_leave: bool,
    hidden_by_user: bool,
    hidden_by_fullscreen: bool,
    time_text: String,
    date_text: String,
    audio: Option<Audio>,
    volume: f32,
    muted: bool,
    dragging_volume: bool,
    brightness: Option<Brightness>,
    brightness_level: f32,
    dragging_brightness: bool,
    page: u8,
    focus_remaining: f32,
    focus_running: bool,
    overlay: Option<Overlay>,
    tasks: tasks::Store,
    shelf: shelf::Store,
    apps: Vec<apps::AppEntry>,
    clip_items: Vec<String>,
    editing: bool,
    edit_text: String,
    ex_style: isize,
    layout: Layout,
    battery: Option<u32>,
    charging: bool,
    wide_grace: Option<Instant>,
    _tray: Tray,
}

impl App {
    fn refresh_clock(&mut self) -> bool {
        let st = unsafe { GetLocalTime() };
        let time = format!("{:02}:{:02}", st.wHour, st.wMinute);
        let date = format!(
            "{}, {} {}",
            weekday(st.wDayOfWeek),
            month(st.wMonth),
            st.wDay
        );
        let changed = time != self.time_text || date != self.date_text;
        self.time_text = time;
        self.date_text = date;
        changed
    }

    fn draw(&mut self) {
        let shape = self.state.shape(&self.metrics);
        if std::env::args().any(|arg| arg == "--trace-shape") {
            log(&format!(
                "shape x={:.1} y={:.1} w={:.1} h={:.1} r={:.1} t={:.2} mode={:?}",
                shape.x, shape.y, shape.w, shape.h, shape.r, self.state.t, self.settings.mode
            ));
        }
        let params = DrawParams {
            shape,
            t: self.state.t,
            scale: self.scale,
            time: self.time_text.clone(),
            date: self.date_text.clone(),
            cpu: self.sysmon.cpu,
            ram: self.sysmon.ram,
            media: media::snapshot(),
            volume: self.volume,
            muted: self.muted,
            brightness: self.brightness_level,
            has_brightness: self.brightness.is_some(),
            net: netstatus::snapshot(),
            page: self.page,
            focus_remaining: self.focus_remaining,
            focus_running: self.focus_running,
            overlay: self.overlay,
            lyrics: lyrics::snapshot(),
            layout: self.layout,
            battery: self.battery,
            charging: self.charging,
            tasks: self.tasks.tasks.clone(),
            shelf_paths: self.shelf.paths.clone(),
            apps: self.apps.iter().take(5).map(|app| app.name.clone()).collect(),
            water: self.tasks.water,
            water_goal: self.tasks.water_goal,
            clip_items: self.clip_items.clone(),
            editing: self.editing,
            edit_text: self.edit_text.clone(),
            opacity: self.settings.opacity.clamp(0.2, 1.0),
            show_clock: self.settings.show_clock,
        };
        if let Err(err) = self.renderer.draw_frame(&params) {
            eprintln!("draw failed: {err}");
            log(&format!("draw failed: {err}"));
        }
    }

    fn set_phase(&mut self, phase: Phase) {
        if self.state.phase == phase {
            return;
        }
        self.state.phase = phase;
        if phase == Phase::Expanded {
            self.sysmon.sample();
            media::set_active(true);
            netstatus::set_active(true);
            lyrics::set_active(true);
            unsafe {
                SetTimer(Some(self.hwnd), TIMER_TICK, 1000, None);
            }
        } else {
            media::set_active(false);
            netstatus::set_active(false);
            lyrics::set_active(false);
            unsafe {
                KillTimer(Some(self.hwnd), TIMER_TICK).ok();
            }
        }
        self.spring
            .set_target(if phase == Phase::Expanded { 1.0 } else { 0.0 });
        if !self.animating {
            self.animating = true;
            self.last_frame = Instant::now();
            arm_anim_timer();
        }
    }

    fn tick_anim(&mut self) {
        if !self.animating {
            return;
        }
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().clamp(0.001, 0.033);
        self.last_frame = now;

        let moving = self.spring.step(dt);
        self.state.t = self.spring.value;
        if self.trace {
            log(&format!("frame {:.2} t={:.4}", dt * 1000.0, self.spring.value));
        }
        self.draw();

        if !moving {
            self.animating = false;
            disarm_anim_timer();
            // A Wide-preset user can open the paged panel temporarily. Once it
            // collapses, return to the wide bar so the preset stays coherent.
            if self.spring.value <= 0.001 && self.layout != self.settings.layout {
                self.apply_layout(self.settings.layout, false);
            }
        }
    }

    fn hit(&self, x: f32, y: f32) -> bool {
        if self.layout == Layout::Cards {
            return state::quick_card().contains(x, y) || state::shelf_card().contains(x, y);
        }
        self.state.hit_shape(&self.metrics).contains(x, y)
    }

    fn hit_with_margin(&self, x: f32, y: f32) -> bool {
        if self.layout == Layout::Cards {
            return self.hit(x, y);
        }
        let mut s = self.state.hit_shape(&self.metrics);
        s.x -= HOVER_MARGIN;
        s.y -= HOVER_MARGIN;
        s.w += HOVER_MARGIN * 2.0;
        s.h += HOVER_MARGIN * 2.0;
        s.contains(x, y)
    }

    fn apply_fullscreen_policy(&mut self) {
        if !self.settings.hide_on_fullscreen {
            return;
        }
        let hide = window::should_hide();
        log(&format!(
            "policy state={} hide={}",
            window::notification_state(),
            hide
        ));
        if hide == self.hidden_by_fullscreen || self.hidden_by_user {
            return;
        }
        self.hidden_by_fullscreen = hide;
        unsafe {
            let _ = ShowWindow(
                self.hwnd,
                if hide { SW_HIDE } else { SW_SHOWNOACTIVATE },
            );
        }
        if !hide {
            self.draw();
        }
    }

    fn set_mode(&mut self, mode: SurfaceMode) {
        if self.settings.mode == mode {
            return;
        }
        self.settings.mode = mode;
        self.settings.save();
        self.metrics = Metrics::for_layout(&self.settings, self.layout);
        self.draw();
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.settings.opacity = opacity.clamp(0.2, 1.0);
        self.settings.save();
        self.draw();
    }

    fn apply_layout(&mut self, layout: Layout, persist: bool) {
        let from_wide = self.layout == Layout::Wide && layout == Layout::Panel;
        self.layout = layout;
        if persist {
            self.settings.layout = layout;
            self.settings.save();
        }
        self.metrics = Metrics::for_layout(&self.settings, layout);
        if layout == Layout::Cards {
            // Cards are a persistent surface, not a hover expansion.
            self.state.phase = Phase::Expanded;
            self.state.t = 1.0;
            self.spring = Spring::new(1.0);
            self.animating = false;
            disarm_anim_timer();
        }
        let width = (self.metrics.surface_w * self.scale).round() as i32;
        let height = (self.metrics.surface_h * self.scale).round() as i32;
        let _ = self.renderer.resize(width as u32, height as u32);
        if layout == Layout::Cards {
            let margin = (16.0 * self.scale).round() as i32;
            let _ = window::place_right(self.hwnd, width, height, margin);
        } else {
            let _ = window::place(self.hwnd, width, height);
        }
        if from_wide {
            self.wide_grace = Some(Instant::now());
        }
        self.draw();
    }

    fn open_panel(&mut self) {
        if self.layout != Layout::Panel {
            self.apply_layout(Layout::Panel, false);
        }
    }

    fn wide_grace_active(&self) -> bool {
        self.wide_grace
            .map(|at| at.elapsed().as_millis() < 800)
            .unwrap_or(false)
    }

    fn toggle_fullscreen_policy(&mut self) {
        self.settings.hide_on_fullscreen = !self.settings.hide_on_fullscreen;
        self.settings.save();
        if self.settings.hide_on_fullscreen {
            self.apply_fullscreen_policy();
        } else if self.hidden_by_fullscreen {
            self.hidden_by_fullscreen = false;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            self.draw();
        }
    }

    fn apply_volume(&mut self, x: f32, rect: &state::Rect) {
        let level = ((x - rect.x) / rect.w).clamp(0.0, 1.0);
        if let Some(audio) = &self.audio {
            audio.set_level(level);
            if self.muted {
                audio.set_muted(false);
            }
        }
        self.volume = level;
        self.muted = false;
        self.draw();
    }

    fn apply_brightness(&mut self, x: f32, rect: &state::Rect) {
        let level = ((x - rect.x) / rect.w).clamp(0.0, 1.0);
        if let Some(brightness) = &self.brightness {
            brightness.set(level);
        }
        self.brightness_level = level;
        self.draw();
    }

    fn show_overlay(&mut self, overlay: Overlay) {
        if self.hidden_by_user || self.hidden_by_fullscreen {
            return;
        }
        if self.state.phase == Phase::Expanded {
            return;
        }
        self.overlay = Some(overlay);
        unsafe {
            SetTimer(Some(self.hwnd), TIMER_OVERLAY, OVERLAY_MS, None);
        }
        self.draw();
    }

    fn update_volume_overlay(&mut self) {
        if self.dragging_volume {
            return;
        }
        if let Some(audio) = &self.audio {
            self.volume = audio.level();
            self.muted = audio.muted();
        }
        let overlay = Overlay {
            kind: OverlayKind::Volume,
            value: self.volume,
            muted: self.muted,
            on: false,
            goal: 0,
        };
        self.show_overlay(overlay);
    }

    fn begin_task_edit(&mut self) {
        if self.editing {
            return;
        }
        self.editing = true;
        self.edit_text.clear();
        unsafe {
            // Keyboard input needs this window to be activatable. It is
            // restored the moment the task is committed or cancelled.
            let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            self.ex_style = style;
            SetWindowLongPtrW(
                self.hwnd,
                GWL_EXSTYLE,
                style & !(WS_EX_NOACTIVATE.0 as isize),
            );
            // SetForegroundWindow alone is refused when the caller is not
            // already foreground; attaching to the foreground input queue
            // makes the transfer legal for the duration of the click.
            let foreground = GetForegroundWindow();
            let foreground_thread = GetWindowThreadProcessId(foreground, None);
            let current_thread = GetCurrentThreadId();
            let _ = AttachThreadInput(current_thread, foreground_thread, true);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(Some(self.hwnd));
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
        self.draw();
    }

    fn end_task_edit(&mut self, commit: bool) {
        if !self.editing {
            return;
        }
        if commit {
            let text = self.edit_text.clone();
            if !text.trim().is_empty() {
                self.tasks.add(&text);
                self.tasks.save();
            }
        }
        self.editing = false;
        self.edit_text.clear();
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let restored = (style & !(WS_EX_NOACTIVATE.0 as isize)) | (self.ex_style & WS_EX_NOACTIVATE.0 as isize);
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, restored);
        }
        self.draw();
    }

    fn toggle_visible(&mut self) {        self.hidden_by_user = !self.hidden_by_user;        unsafe {
            let _ = ShowWindow(
                self.hwnd,
                if self.hidden_by_user {
                    SW_HIDE
                } else {
                    SW_SHOWNOACTIVATE
                },
            );
        }
        if !self.hidden_by_user {
            self.draw();
        }
    }
}

fn app(hwnd: HWND) -> Option<&'static mut App> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        ptr.as_mut()
    }
}

fn arm_anim_timer() {
    let raw = ANIM_TIMER.load(Ordering::Relaxed);
    if raw == 0 {
        return;
    }
    let due: i64 = -(FRAME_MS * 10_000);
    unsafe {
        let _ = SetWaitableTimer(
            HANDLE(raw as *mut core::ffi::c_void),
            &due,
            FRAME_MS as i32,
            None,
            None,
            false,
        );
    }
}

fn disarm_anim_timer() {
    let raw = ANIM_TIMER.load(Ordering::Relaxed);
    if raw == 0 {
        return;
    }
    unsafe {
        let _ = CancelWaitableTimer(HANDLE(raw as *mut core::ffi::c_void));
    }
}

fn x_lparam(lparam: LPARAM) -> f32 {
    (lparam.0 as u16 as i16) as i32 as f32
}

fn y_lparam(lparam: LPARAM) -> f32 {
    ((lparam.0 >> 16) as u16 as i16) as i32 as f32
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let hwnd = NOTCH_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        let _ = PostMessageW(
            Some(HWND(hwnd as *mut _)),
            WM_APP_REFRESH,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

unsafe extern "system" fn notch_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let settings = Settings::load();
            let metrics = Metrics::from_settings(&settings);
            let scale = window::scale_for(hwnd);
            let width = (metrics.surface_w * scale).round() as u32;
            let height = (metrics.surface_h * scale).round() as u32;

            // The composition target only presents when the window is already
            // visible, so show it before creating the renderer.
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

            let renderer = match Renderer::new(hwnd, width, height) {
                Ok(renderer) => renderer,
                Err(err) => {
                    eprintln!("renderer init failed: {err}");
                    return LRESULT(-1);
                }
            };
            let tray_owner = HWND(MENU_OWNER.load(Ordering::Relaxed) as *mut core::ffi::c_void);
            let tray = match Tray::new(tray_owner) {
                Ok(tray) => tray,
                Err(err) => {
                    eprintln!("tray init failed: {err}");
                    return LRESULT(-1);
                }
            };
            media::start(hwnd);
            netstatus::start(hwnd);
            clipboard::start(hwnd);
            lyrics::start(hwnd);
            unsafe {
                DragAcceptFiles(hwnd, true);
            }

            let audio = Audio::new(hwnd).ok();
            let volume = audio.as_ref().map(|a| a.level()).unwrap_or(0.0);
            let muted = audio.as_ref().map(|a| a.muted()).unwrap_or(false);
            let brightness = Brightness::new(hwnd);
            let brightness_level = brightness.as_ref().map(|b| b.level()).unwrap_or(0.0);

            let trace = std::env::args().any(|arg| arg == "--trace-anim");
            let layout = settings.layout;
            let (battery, charging) = battery_state();
            let mut app = Box::new(App {
                hwnd,
                renderer,
                state: AppState::new(),
                metrics,
                settings,
                sysmon: SysMon::new(),
                scale,
                spring: Spring::new(0.0),
                animating: false,
                last_frame: Instant::now(),
                trace,
                trace_start: Instant::now(),
                trace_stage: 0,
                tracking_leave: false,
                hidden_by_user: false,
                hidden_by_fullscreen: false,
                time_text: String::new(),
                date_text: String::new(),
                audio,
                volume,
                muted,
                dragging_volume: false,
                brightness,
                brightness_level,
                dragging_brightness: false,
                page: 0,
                focus_remaining: 1500.0,
                focus_running: false,
                overlay: None,
                tasks: tasks::Store::load(),
                shelf: shelf::Store::load(),
                apps: apps::load(),
                clip_items: clipboard::snapshot(),
                editing: false,
                edit_text: String::new(),
                ex_style: 0,
                layout,
                battery,
                charging,
                wide_grace: None,
                _tray: tray,
            });
            app.refresh_clock();
            if std::env::args().any(|arg| arg == "--expanded") {
                app.state.phase = Phase::Expanded;
                app.state.t = 1.0;
                app.spring = Spring::new(1.0);
                media::set_active(true);
                netstatus::set_active(true);
                lyrics::set_active(true);
                unsafe {
                    SetTimer(Some(hwnd), TIMER_TICK, 1000, None);
                }
            }
            if trace {
                unsafe {
                    SetTimer(Some(hwnd), TIMER_TRACE, 50, None);
                }
            }
            unsafe {
                SetTimer(Some(hwnd), TIMER_WATER, 45 * 60 * 1000, None);
            }
            let ptr = Box::into_raw(app);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
            NOTCH_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

            if layout == Layout::Cards {
                (*ptr).state.phase = Phase::Expanded;
                (*ptr).state.t = 1.0;
                (*ptr).spring = Spring::new(1.0);
            }
            (*ptr).draw();
            let placed = if layout == Layout::Cards {
                window::place_right(hwnd, width as i32, height as i32, (16.0 * scale) as i32)
            } else {
                window::place(hwnd, width as i32, height as i32)
            };
            log(&format!(
                "created {}x{} scale={} place={:?} state={} hide={}",
                width,
                height,
                scale,
                placed.is_ok(),
                window::notification_state(),
                window::should_hide()
            ));
            LRESULT(0)
        }
        WM_NCHITTEST => {
            if let Some(app) = app(hwnd) {
                let mut pt = POINT {
                    x: x_lparam(lparam) as i32,
                    y: y_lparam(lparam) as i32,
                };
                let _ = ScreenToClient(hwnd, &mut pt);
                if app.hit(pt.x as f32 / app.scale, pt.y as f32 / app.scale) {
                    return LRESULT(HTCLIENT as isize);
                }
            }
            LRESULT(HTTRANSPARENT as isize)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => {
            if let Some(app) = app(hwnd) {
                let x = x_lparam(lparam) / app.scale;
                let y = y_lparam(lparam) / app.scale;

                if app.layout == Layout::Cards {
                    let hits = state::card_hits();
                    if app.dragging_volume {
                        app.apply_volume(x, &hits.volume);
                    } else if app.dragging_brightness {
                        app.apply_brightness(x, &hits.brightness);
                    }
                    return LRESULT(0);
                }

                if app.dragging_volume {
                    let shape = app.state.shape(&app.metrics);
                    let controls = state::controls(&shape);
                    app.apply_volume(x, &controls.volume);
                } else if app.dragging_brightness {
                    let shape = app.state.shape(&app.metrics);
                    let controls = state::controls(&shape);
                    app.apply_brightness(x, &controls.brightness);
                } else {
                    if !app.tracking_leave {
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);
                        app.tracking_leave = true;
                    }

                    match app.state.phase {
                        Phase::Compact if app.hit(x, y) => app.set_phase(Phase::Expanded),
                        Phase::Expanded
                            if !app.hit_with_margin(x, y) && !app.wide_grace_active() =>
                        {
                            app.set_phase(Phase::Compact);
                        }
                        _ => {}
                    }
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if let Some(app) = app(hwnd) {
                if app.editing {
                    match wparam.0 as u16 {
                        code if code == windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN.0 => {
                            app.end_task_edit(true);
                        }
                        code if code == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 => {
                            app.end_task_edit(false);
                        }
                        code if code == windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK.0 => {
                            app.edit_text.pop();
                            app.draw();
                        }
                        _ => {}
                    }
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CHAR => {
            if let Some(app) = app(hwnd) {
                if app.editing {
                    if let Some(ch) = char::from_u32(wparam.0 as u32) {
                        if !ch.is_control() && app.edit_text.chars().count() < 64 {
                            app.edit_text.push(ch);
                            app.draw();
                        }
                    }
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KILLFOCUS => {
            if let Some(app) = app(hwnd) {
                if app.editing {
                    app.end_task_edit(true);
                }
            }
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            if let Some(app) = app(hwnd) {
                clipboard::ingest(hwnd);
                let next = clipboard::snapshot();
                if next != app.clip_items {
                    app.clip_items = next;
                    app.draw();
                }
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            if let Some(app) = app(hwnd) {
                let drop = HDROP(wparam.0 as *mut core::ffi::c_void);
                let mut point = POINT::default();
                let _ = DragQueryPoint(drop, &mut point);
                let inside = app.hit_with_margin(
                    point.x as f32 / app.scale,
                    point.y as f32 / app.scale,
                );
                let mut paths = Vec::new();
                if inside {
                    let count = DragQueryFileW(drop, u32::MAX, None);
                    for index in 0..count {
                        let len = DragQueryFileW(drop, index, None);
                        if len == 0 {
                            continue;
                        }
                        let mut buffer = vec![0u16; len as usize + 1];
                        let copied = DragQueryFileW(drop, index, Some(&mut buffer));
                        if copied > 0 {
                            paths.push(String::from_utf16_lossy(&buffer[..copied as usize]));
                        }
                    }
                }
                DragFinish(drop);
                if !paths.is_empty() {
                    app.shelf.add(paths);
                    app.page = 4;
                    app.set_phase(Phase::Expanded);
                    app.draw();
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {            if let Some(app) = app(hwnd) {
                let x = x_lparam(lparam) / app.scale;
                let y = y_lparam(lparam) / app.scale;
                if app.layout == Layout::Cards {
                    let hits = state::card_hits();
                    if hits.volume.contains(x, y) {
                        app.dragging_volume = true;
                        let _ = SetCapture(hwnd);
                        app.apply_volume(x, &hits.volume);
                    } else if hits.brightness.contains(x, y) {
                        app.dragging_brightness = true;
                        let _ = SetCapture(hwnd);
                        app.apply_brightness(x, &hits.brightness);
                    }
                    return LRESULT(0);
                }
                if app.state.phase == Phase::Expanded && app.layout == Layout::Panel && app.page == 1 {
                    let shape = app.state.shape(&app.metrics);
                    let controls = state::controls(&shape);
                    if controls.volume.contains(x, y) {
                        app.dragging_volume = true;
                        let _ = SetCapture(hwnd);
                        app.apply_volume(x, &controls.volume);
                    } else if controls.brightness.contains(x, y) {
                        app.dragging_brightness = true;
                        let _ = SetCapture(hwnd);
                        app.apply_brightness(x, &controls.brightness);
                    }
                }
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if let Some(app) = app(hwnd) {
                app.tracking_leave = false;
                if !app.editing {
                    app.set_phase(Phase::Compact);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(app) = app(hwnd) {
                let x = x_lparam(lparam) / app.scale;
                let y = y_lparam(lparam) / app.scale;
                if app.layout == Layout::Cards {
                    let hits = state::card_hits();
                    if app.dragging_volume {
                        app.apply_volume(x, &hits.volume);
                        app.dragging_volume = false;
                        let _ = ReleaseCapture();
                        return LRESULT(0);
                    }
                    if app.dragging_brightness {
                        app.apply_brightness(x, &hits.brightness);
                        app.dragging_brightness = false;
                        let _ = ReleaseCapture();
                        return LRESULT(0);
                    }
                    if hits.focus.contains(x, y) {
                        app.focus_running = !app.focus_running;
                        if app.focus_running {
                            app.focus_remaining = app.focus_remaining.max(1.0);
                        }
                        app.draw();
                        return LRESULT(0);
                    }
                    if hits.battery.contains(x, y) {
                        open_uri(w!("ms-settings:batterysaver"));
                        return LRESULT(0);
                    }
                    if hits.record.contains(x, y) {
                        open_uri(w!("ms-gamebar"));
                        return LRESULT(0);
                    }
                    if hits.theme.contains(x, y) {
                        open_uri(w!("ms-settings:colors"));
                        return LRESULT(0);
                    }
                    if hits.clear.contains(x, y) {
                        app.shelf.clear();
                        app.draw();
                        return LRESULT(0);
                    }
                    for (index, row) in hits.rows.iter().enumerate() {
                        if row.contains(x, y) {
                            if let Some(path) = app.shelf.paths.get(index) {
                                open_path(path);
                            }
                            return LRESULT(0);
                        }
                    }
                    return LRESULT(0);
                }
                if app.dragging_volume {
                    let shape = app.state.shape(&app.metrics);
                    let controls = state::controls(&shape);
                    app.apply_volume(x, &controls.volume);
                    app.dragging_volume = false;
                    let _ = ReleaseCapture();
                    return LRESULT(0);
                }
                if app.dragging_brightness {
                    let shape = app.state.shape(&app.metrics);
                    let controls = state::controls(&shape);
                    app.apply_brightness(x, &controls.brightness);
                    app.dragging_brightness = false;
                    let _ = ReleaseCapture();
                    return LRESULT(0);
                }
                if app.state.phase == Phase::Expanded && app.state.t > 0.5 {
                    let shape = app.state.shape(&app.metrics);
                    if app.layout == Layout::Wide {
                        let wide = state::wide_layout(&shape);
                        let cmd = if wide.prev.contains(x, y) {
                            Some(media::Command::Previous)
                        } else if wide.play.contains(x, y) {
                            Some(media::Command::Toggle)
                        } else if wide.next.contains(x, y) {
                            Some(media::Command::Next)
                        } else {
                            None
                        };
                        if let Some(cmd) = cmd {
                            media::send(cmd);
                            return LRESULT(0);
                        }
                        if wide.progress.contains(x, y) {
                            let ratio = ((x - wide.progress.x) / wide.progress.w).clamp(0.0, 1.0);
                            let duration = media::snapshot().duration;
                            if duration > 0.0 {
                                media::send(media::Command::Seek(duration * ratio));
                            }
                            return LRESULT(0);
                        }
                        app.open_panel();
                        return LRESULT(0);
                    }
                    if app.editing && !state::task_add(&shape).contains(x, y) {
                        app.end_task_edit(true);
                    }
                    let tabs = state::tabs(&shape);
                    if tabs.overview.contains(x, y) {
                        app.page = 0;
                        app.draw();
                        return LRESULT(0);
                    }
                    if tabs.controls.contains(x, y) {
                        app.page = 1;
                        app.draw();
                        return LRESULT(0);
                    }
                    if tabs.focus.contains(x, y) {
                        app.page = 2;
                        app.draw();
                        return LRESULT(0);
                    }
                    if tabs.tasks.contains(x, y) {
                        app.page = 3;
                        app.draw();
                        return LRESULT(0);
                    }
                    if tabs.shelf.contains(x, y) {
                        app.page = 4;
                        app.draw();
                        return LRESULT(0);
                    }
                    if tabs.clip.contains(x, y) {
                        app.page = 5;
                        app.draw();
                        return LRESULT(0);
                    }
                    if tabs.apps.contains(x, y) {
                        app.page = 6;
                        app.draw();
                        return LRESULT(0);
                    }
                    if app.page == 3 {
                        if state::task_add(&shape).contains(x, y) {
                            app.begin_task_edit();
                            return LRESULT(0);
                        }
                        for index in 0..4 {
                            if state::task_row(&shape, index).contains(x, y)
                                && index < app.tasks.tasks.len()
                            {
                                app.tasks.toggle(index);
                                app.tasks.save();
                                app.draw();
                                return LRESULT(0);
                            }
                        }
                        for index in 0..8 {
                            if state::water_dot(&shape, index).contains(x, y) {
                                app.tasks.roll_day();
                                let next = index as u32 + 1;
                                app.tasks.water = if app.tasks.water == next {
                                    index as u32
                                } else {
                                    next
                                };
                                app.tasks.save();
                                app.draw();
                                return LRESULT(0);
                            }
                        }
                    }
                    if app.page == 4 {
                        if state::clip_clear(&shape).contains(x, y) {
                            app.shelf.clear();
                            app.draw();
                            return LRESULT(0);
                        }
                        for index in 0..5 {
                            if state::clip_row(&shape, index).contains(x, y) {
                                if let Some(path) = app.shelf.paths.get(index) {
                                    open_path(path);
                                }
                                return LRESULT(0);
                            }
                        }
                    }
                    if app.page == 5 {
                        if state::clip_clear(&shape).contains(x, y) {
                            clipboard::clear();
                            app.clip_items.clear();
                            app.draw();
                            return LRESULT(0);
                        }
                        for index in 0..5 {
                            if state::clip_row(&shape, index).contains(x, y) {
                                if let Some(text) = app.clip_items.get(index) {
                                    clipboard::copy(hwnd, text);
                                    app.show_overlay(Overlay {
                                        kind: OverlayKind::Copied,
                                        value: 0.0,
                                        muted: false,
                                        on: false,
                                        goal: 0,
                                    });
                                }
                                return LRESULT(0);
                            }
                        }
                    }
                    if app.page == 6 {
                        for index in 0..5 {
                            if state::clip_row(&shape, index).contains(x, y) {
                                if let Some(entry) = app.apps.get(index) {
                                    open_path(&entry.path);
                                }
                                return LRESULT(0);
                            }
                        }
                    }
                    if app.page == 2 {
                        if state::focus_toggle(&shape).contains(x, y) {
                            app.focus_running = !app.focus_running;
                            app.draw();
                            return LRESULT(0);
                        }
                        if state::focus_reset(&shape).contains(x, y) {
                            app.focus_remaining = 1500.0;
                            app.focus_running = false;
                            app.draw();
                            return LRESULT(0);
                        }
                    }
                    if app.page == 0 {
                        let progress = state::progress_bar(&shape);
                        if progress.contains(x, y) {
                            let ratio = ((x - progress.x) / progress.w).clamp(0.0, 1.0);
                            let duration = media::snapshot().duration;
                            if duration > 0.0 {
                                media::send(media::Command::Seek(duration * ratio));
                            }
                            return LRESULT(0);
                        }
                    }
                    let buttons = state::transport(&shape);
                    let cmd = if buttons.prev.contains(x, y) {
                        Some(media::Command::Previous)
                    } else if buttons.play.contains(x, y) {
                        Some(media::Command::Toggle)
                    } else if buttons.next.contains(x, y) {
                        Some(media::Command::Next)
                    } else {
                        None
                    };
                    if let Some(cmd) = cmd {
                        media::send(cmd);
                    }
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if let Some(app) = app(hwnd) {
                match wparam.0 {
                    TIMER_TICK => {
                        if app.refresh_clock() {
                            app.draw();
                        }
                        if app.state.phase == Phase::Expanded {
                            app.sysmon.sample();
                            if let Some(audio) = &app.audio {
                                app.volume = audio.level();
                                app.muted = audio.muted();
                            }
                            if let Some(brightness) = &app.brightness {
                                app.brightness_level = brightness.level();
                            }
                            if app.focus_running {
                                app.focus_remaining = (app.focus_remaining - 1.0).max(0.0);
                                if app.focus_remaining <= 0.0 {
                                    app.focus_running = false;
                                }
                            }
                            let (battery, charging) = battery_state();
                            app.battery = battery;
                            app.charging = charging;
                            app.draw();
                        }
                    }
                    TIMER_TRACE => {
                        let elapsed = app.trace_start.elapsed().as_secs_f32();
                        match app.trace_stage {
                            0 if elapsed > 0.6 => {
                                app.set_phase(Phase::Expanded);
                                app.trace_stage = 1;
                            }
                            1 if elapsed > 1.6 => {
                                app.set_phase(Phase::Compact);
                                app.trace_stage = 2;
                            }
                            2 if elapsed > 2.6 => PostQuitMessage(0),
                            _ => {}
                        }
                    }
                    TIMER_OVERLAY => {
                        unsafe {
                            KillTimer(Some(hwnd), TIMER_OVERLAY).ok();
                        }
                        if app.overlay.take().is_some() {
                            app.draw();
                        }
                    }
                    TIMER_WATER => {
                        app.tasks.roll_day();
                        if app.tasks.water < app.tasks.water_goal {
                            let goal = app.tasks.water_goal;
                            app.show_overlay(Overlay {
                                kind: OverlayKind::Water,
                                value: app.tasks.water as f32 / goal as f32,
                                muted: false,
                                on: false,
                                goal,
                            });
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            if let Some(app) = app(hwnd) {
                app.draw();
            }
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_APP_REDRAW => {
            if let Some(app) = app(hwnd) {
                app.draw();
            }
            LRESULT(0)
        }
        media::WM_APP_MEDIA => {
            if let Some(app) = app(hwnd) {
                app.draw();
            }
            LRESULT(0)
        }
        netstatus::WM_APP_NET => {
            if let Some(app) = app(hwnd) {
                app.draw();
            }
            LRESULT(0)
        }
        lyrics::WM_APP_LYRICS => {
            if let Some(app) = app(hwnd) {
                app.draw();
            }
            LRESULT(0)
        }
        audio::WM_APP_VOLUME => {
            if let Some(app) = app(hwnd) {
                app.update_volume_overlay();
                if app.layout == Layout::Cards {
                    app.draw();
                }
            }
            LRESULT(0)
        }
        WM_APP_CAPS => {
            if let Some(app) = app(hwnd) {
                let on = unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0 };
                app.show_overlay(Overlay {
                    kind: OverlayKind::Caps,
                    value: 0.0,
                    muted: false,
                    on,
                    goal: 0,
                });
            }
            LRESULT(0)
        }
        WM_APP_REFRESH => {
            if let Some(app) = app(hwnd) {
                app.apply_fullscreen_policy();
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            if let Some(app) = app(hwnd) {
                app.scale = window::scale_for(hwnd);
                let width = (app.metrics.surface_w * app.scale).round() as i32;
                let height = (app.metrics.surface_h * app.scale).round() as i32;
                let _ = app.renderer.resize(width as u32, height as u32);
                if app.layout == Layout::Cards {
                    let margin = (16.0 * app.scale).round() as i32;
                    let _ = window::place_right(hwnd, width, height, margin);
                } else {
                    let _ = window::place(hwnd, width, height);
                }
                app.draw();
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
            if !ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(ptr));
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn menu_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        tray::WM_TRAY => {
            let event = (lparam.0 as u16 as i16) as i32;
            if event == WM_RBUTTONUP as i32 || event == WM_CONTEXTMENU as i32 {
                let notch = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void))
                    .map(|app| app.settings.mode == SurfaceMode::Notch)
                    .unwrap_or(false);
                let menu_state = {
                    let app = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void));
                    tray::MenuState {
                        notch,
                        autostart: tray::autostart_enabled(),
                        opacity: app
                            .as_ref()
                            .map(|app| app.settings.opacity)
                            .unwrap_or(1.0),
                        hide_on_fullscreen: app
                            .as_ref()
                            .map(|app| app.settings.hide_on_fullscreen)
                            .unwrap_or(true),
                        layout: app
                            .as_ref()
                            .map(|app| app.layout)
                            .unwrap_or_default(),
                    }
                };
                let _ = tray::show_menu(hwnd, &menu_state);
            } else if event == WM_LBUTTONUP as i32 {
                if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                    app.toggle_visible();
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            match id {
                tray::CMD_TOGGLE => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.toggle_visible();
                    }
                }
                tray::CMD_SETTINGS => {
                    open_settings_folder();
                }
                tray::CMD_MODE_NOTCH => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.set_mode(SurfaceMode::Notch);
                    }
                }
                tray::CMD_MODE_ISLAND => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.set_mode(SurfaceMode::Island);
                    }
                }
                tray::CMD_AUTOSTART => {
                    let enabled = tray::autostart_enabled();
                    tray::set_autostart(!enabled);
                }
                tray::CMD_OPACITY_80 => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.set_opacity(0.8);
                    }
                }
                tray::CMD_OPACITY_90 => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.set_opacity(0.9);
                    }
                }
                tray::CMD_OPACITY_100 => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.set_opacity(1.0);
                    }
                }
                tray::CMD_FULLSCREEN => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.toggle_fullscreen_policy();
                    }
                }
                tray::CMD_LAYOUT_PANEL => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.apply_layout(Layout::Panel, true);
                    }
                }
                tray::CMD_LAYOUT_WIDE => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.apply_layout(Layout::Wide, true);
                    }
                }
                tray::CMD_LAYOUT_CARDS => {
                    if let Some(app) = app(HWND(NOTCH_HWND.load(Ordering::Relaxed) as *mut core::ffi::c_void)) {
                        app.apply_layout(Layout::Cards, true);
                    }
                }
                tray::CMD_QUIT => {
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn open_settings_folder() {
    let path = Settings::dir();
    let _ = std::fs::create_dir_all(&path);
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR::from_raw(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

fn open_path(path: &str) {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR::from_raw(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

fn battery_state() -> (Option<u32>, bool) {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_ok() && status.BatteryLifePercent != 255 {
        (
            Some(status.BatteryLifePercent as u32),
            status.ACLineStatus == 1,
        )
    } else {
        (None, false)
    }
}

fn open_uri(uri: PCWSTR) {
    unsafe {
        ShellExecuteW(None, w!("open"), uri, None, None, SW_SHOWNORMAL);
    }
}

fn weekday(day: u16) -> &'static str {
    match day {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    }
}

fn month(month: u16) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_KEYUP {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if info.vkCode == VK_CAPITAL.0 as u32 {
            let raw = NOTCH_HWND.load(Ordering::Relaxed);
            if raw != 0 {
                let _ = PostMessageW(
                    Some(HWND(raw as *mut core::ffi::c_void)),
                    WM_APP_CAPS,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn run() -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let _guard = CreateMutexW(None, true, w!("Local\\NotchSingleInstance"))?;
        if windows::Win32::Foundation::GetLastError() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS
        {
            return Ok(());
        }

        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(notch_wndproc),
            hInstance: HINSTANCE(instance.0),
            lpszClassName: w!("NotchSurface"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(windows::core::Error::from_thread());
        }

        let menu_class = WNDCLASSW {
            lpfnWndProc: Some(menu_wndproc),
            hInstance: HINSTANCE(instance.0),
            lpszClassName: w!("NotchTrayHost"),
            ..Default::default()
        };
        if RegisterClassW(&menu_class) == 0 {
            return Err(windows::core::Error::from_thread());
        }

        let _menu_owner = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("NotchTrayHost"),
            w!("NOTCH tray"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )?;
        MENU_OWNER.store(_menu_owner.0 as isize, Ordering::Relaxed);

        let _hwnd = CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("NotchSurface"),
            w!("NOTCH"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )?;

        let _hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );

        // Low-level hook only observes toggles; it never swallows a key.
        let _key_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0).ok();

        let timer = CreateWaitableTimerExW(
            None,
            None,
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ALL_ACCESS.0,
        )?;
        ANIM_TIMER.store(timer.0 as isize, Ordering::Relaxed);

        let notch_hwnd = _hwnd;
        let _ = PostMessageW(Some(notch_hwnd), WM_APP_REDRAW, WPARAM(0), LPARAM(0));

        let mut msg = MSG::default();
        loop {
            let wait = MsgWaitForMultipleObjectsEx(
                Some(&[timer]),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
            if wait == WAIT_OBJECT_0 {
                if let Some(app) = app(notch_hwnd) {
                    app.tick_anim();
                }
            } else if wait == WAIT_EVENT(WAIT_OBJECT_0.0 + 1) {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        return Ok(());
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            } else {
                break;
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("notch failed: {err}");
        std::process::exit(1);
    }
}
