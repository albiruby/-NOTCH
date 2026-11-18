//! DirectComposition + Direct2D + DirectWrite surface.

use windows::core::{Interface, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D1_GRADIENT_STOP, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1Brush, ID2D1Device, ID2D1DeviceContext, ID2D1LinearGradientBrush,
    ID2D1SolidColorBrush, ID2D1Bitmap, D2D1_LAYER_PARAMETERS1, D2D1_LAYER_OPTIONS1_NONE, D2D1_ELLIPSE, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_BUFFER_PRECISION_8BPC_UNORM,
    D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT, D2D1_COLOR_SPACE_SRGB,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_EXTEND_MODE_CLAMP,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows_numerics::Vector2;

use crate::state::Shape;
use crate::theme;

pub struct DrawParams {
    pub shape: Shape,
    pub t: f32,
    pub scale: f32,
    pub time: String,
    pub date: String,
    pub cpu: f32,
    pub ram: f32,
    pub media: crate::media::MediaInfo,
    pub volume: f32,
    pub muted: bool,
    pub brightness: f32,
    pub has_brightness: bool,
    pub net: crate::netstatus::NetStatus,
    pub page: u8,
    pub focus_remaining: f32,
    pub focus_running: bool,
    pub overlay: Option<crate::state::Overlay>,
    pub lyrics: crate::lyrics::Lyrics,
    pub layout: crate::state::Layout,
    pub battery: Option<u32>,
    pub charging: bool,
    pub tasks: Vec<crate::tasks::Task>,
    pub shelf_paths: Vec<String>,
    pub apps: Vec<String>,
    pub water: u32,
    pub water_goal: u32,
    pub clip_items: Vec<String>,
    pub editing: bool,
    pub edit_text: String,
    pub opacity: f32,
    pub show_clock: bool,
    pub mode: crate::state::SurfaceMode,
}

pub struct Renderer {
    _d3d: ID3D11Device,
    _d2d: ID2D1Device,
    dcomp: IDCompositionDevice,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual,
    factory: IDXGIFactory2,
    swapchain: IDXGISwapChain1,
    ctx: ID2D1DeviceContext,
    _bitmap: ID2D1Bitmap1,
    width: u32,
    height: u32,
    fmt_clock: IDWriteTextFormat,
    fmt_big: IDWriteTextFormat,
    fmt_label: IDWriteTextFormat,
    fmt_value: IDWriteTextFormat,
    fmt_wordmark: IDWriteTextFormat,
    fmt_media_title: IDWriteTextFormat,
    fmt_media_artist: IDWriteTextFormat,
    fmt_time_left: IDWriteTextFormat,
    fmt_time_right: IDWriteTextFormat,
    fmt_status: IDWriteTextFormat,
    fmt_tab: IDWriteTextFormat,
    fmt_focus: IDWriteTextFormat,
    fmt_body: IDWriteTextFormat,
    fmt_small: IDWriteTextFormat,
    fmt_wide_time: IDWriteTextFormat,
    fmt_wide_title: IDWriteTextFormat,
    fmt_wide_sub: IDWriteTextFormat,
    fmt_icon: IDWriteTextFormat,
    fmt_icon_sm: IDWriteTextFormat,
    fmt_logo: IDWriteTextFormat,
    fmt_pill_time: IDWriteTextFormat,
    fmt_pill_title: IDWriteTextFormat,
    fmt_card_title: IDWriteTextFormat,
    fmt_card_title_center: IDWriteTextFormat,
    art_id: u64,
    art_bitmap: Option<ID2D1Bitmap1>,
}

impl Renderer {
    pub fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        unsafe {
            let mut d3d: Option<ID3D11Device> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )?;
            let d3d = d3d.unwrap();
            let dxgi: IDXGIDevice = d3d.cast()?;
            let d2d: ID2D1Device = windows::Win32::Graphics::Direct2D::D2D1CreateDevice(&dxgi, None)?;

            let dcomp: IDCompositionDevice = DCompositionCreateDevice2(&d2d)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            target.SetRoot(&visual)?;

            let adapter = dxgi.GetAdapter()?;
            let factory: IDXGIFactory2 = adapter.GetParent()?;
            let swapchain =
                factory.CreateSwapChainForComposition(&d3d, &swapchain_desc(width, height), None)?;
            let surface: IDXGISurface = swapchain.GetBuffer(0)?;
            let ctx = d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let bitmap = ctx.CreateBitmapFromDxgiSurface(&surface, None)?;
            ctx.SetTarget(&bitmap);
            visual.SetContent(&swapchain)?;
            dcomp.Commit()?;

            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            Ok(Self {
                _d3d: d3d,
                _d2d: d2d,
                dcomp,
                _target: target,
                visual,
                factory,
                swapchain,
                ctx,
                _bitmap: bitmap,
                width,
                height,
                fmt_clock: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::BODY_SM,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_big: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::DISPLAY,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_label: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::LABEL,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_value: format(
                    &dwrite,
                    theme::typography::FONT_MONO,
                    theme::typography::LABEL,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_wordmark: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::MICRO,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_media_title: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    13.0,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_media_artist: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::CAPTION,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_time_left: format(
                    &dwrite,
                    theme::typography::FONT_MONO,
                    theme::typography::CAPTION,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_time_right: format(
                    &dwrite,
                    theme::typography::FONT_MONO,
                    theme::typography::CAPTION,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_status: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::MICRO,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_tab: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::MICRO,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_focus: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    46.0,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_body: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::LABEL,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_small: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::CAPTION,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_wide_time: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    22.0,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_wide_title: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    14.0,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_wide_sub: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::CAPTION,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                )?,
                fmt_icon: format(
                    &dwrite,
                    "Segoe MDL2 Assets",
                    15.0,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_icon_sm: format(
                    &dwrite,
                    "Segoe MDL2 Assets",
                    12.0,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_logo: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    13.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_pill_time: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::LABEL,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_pill_title: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    theme::typography::CAPTION,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_card_title: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    14.0,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                fmt_card_title_center: format(
                    &dwrite,
                    theme::typography::FONT_UI,
                    16.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )?,
                art_id: 0,
                art_bitmap: None,
            })
        }
    }

    pub fn draw_frame(&mut self, p: &DrawParams) -> Result<()> {
        let s = p.scale;
        let shape = p.shape;

        if p.media.art_id != self.art_id {
            self.art_id = p.media.art_id;
            self.art_bitmap = p
                .media
                .art
                .as_deref()
                .and_then(|bytes| decode_art(&self.ctx, bytes).ok());
        }

        unsafe {
            let ctx = &self.ctx;
            ctx.BeginDraw();
            ctx.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            ctx.Clear(None);

            if p.layout == crate::state::Layout::Cards {
                self.draw_cards(p, None)?;
                ctx.EndDraw(None, None)?;
                let hr = self.swapchain.Present(0, DXGI_PRESENT(0));
                hr.ok()?;
                return Ok(());
            }

            if p.layout == crate::state::Layout::Hub && p.t > 0.01 {
                self.draw_hub(p)?;
            }

            let border = theme::rgb(theme::accent::c800(), 0.30 * p.opacity);
            let primary = theme::rgb(theme::accent::text1(), 1.0);
            let secondary = theme::rgb(theme::accent::text2(), 1.0);
            let accent = theme::rgb(theme::accent::c600(), 1.0);
            let track = theme::rgb(theme::accent::track(), 1.0);

            let rect = D2D_RECT_F {
                left: shape.x * s,
                top: shape.y * s,
                right: shape.right() * s,
                bottom: shape.bottom() * s,
            };
            let radius = shape.r * s;
            if p.mode.solid() {
                draw_hardware(ctx, rect, radius, p.mode, s)?;
            } else {
                draw_glass(ctx, rect, radius, p.opacity, Glass::Surface, s)?;
            }

            // Shape coordinates can start above the surface in notch mode, so
            // content is laid out from the visible top edge.
            let visible_top = shape.y.max(0.0);

            // Compact content fades out while the panel grows.
            let compact_alpha = (1.0 - p.t).clamp(0.0, 1.0);
            if compact_alpha > 0.01 {
                if let Some(overlay) = p.overlay {
                    let text_brush = solid(ctx, &primary)?;
                    text_brush.SetOpacity(compact_alpha);
                    let label = match overlay.kind {
                        crate::state::OverlayKind::Volume => {
                            if overlay.muted {
                                "MUTED".to_string()
                            } else {
                                format!("VOLUME  {:.0}%", overlay.value * 100.0)
                            }
                        }
                        crate::state::OverlayKind::Caps => {
                            if overlay.on {
                                "CAPS LOCK  ON".to_string()
                            } else {
                                "CAPS LOCK  OFF".to_string()
                            }
                        }
                        crate::state::OverlayKind::Water => {
                            let done = (overlay.value * overlay.goal as f32).round() as u32;
                            format!("DRINK WATER  {done}/{}", overlay.goal)
                        }
                        crate::state::OverlayKind::Copied => "COPIED".to_string(),
                    };
                    let has_bar = matches!(
                        overlay.kind,
                        crate::state::OverlayKind::Volume | crate::state::OverlayKind::Water
                    );
                    let text_top = if has_bar {
                        visible_top + 3.0
                    } else {
                        visible_top + 2.0
                    };
                    draw_text(
                        ctx,
                        &label,
                        &self.fmt_clock,
                        D2D_RECT_F {
                            left: (shape.x + 8.0) * s,
                            top: text_top * s,
                            right: (shape.right() - 8.0) * s,
                            bottom: (shape.bottom() - 10.0) * s,
                        },
                        &text_brush,
                    );

                    if matches!(
                        overlay.kind,
                        crate::state::OverlayKind::Volume | crate::state::OverlayKind::Water
                    ) {
                        let bar_w = (shape.w - 56.0).max(40.0);
                        let bar_left = shape.x + (shape.w - bar_w) * 0.5;
                        let bar_y = shape.bottom() - 8.5;
                        let track_brush = solid(ctx, &track)?;
                        track_brush.SetOpacity(0.9 * compact_alpha);
                        ctx.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: scaled_rect_raw(bar_left, bar_y, bar_w, 2.5, s),
                                radiusX: 1.25 * s,
                                radiusY: 1.25 * s,
                            },
                            &track_brush,
                        );
                        if overlay.value > 0.001 && !overlay.muted {
                            let fill_brush = solid(ctx, &accent)?;
                            fill_brush.SetOpacity(compact_alpha);
                            ctx.FillRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: scaled_rect_raw(
                                        bar_left,
                                        bar_y,
                                        bar_w * overlay.value.clamp(0.0, 1.0),
                                        2.5,
                                        s,
                                    ),
                                    radiusX: 1.25 * s,
                                    radiusY: 1.25 * s,
                                },
                                &fill_brush,
                            );
                        }
                    }
                } else if p.show_clock {
                    // Brand mark left, now-playing in the middle, clock right,
                    // mirroring the reference top pill.
                    let center_y = (visible_top + shape.bottom()) * 0.5;
                    let mark_r = ((shape.bottom() - visible_top) * 0.5 - 6.0).clamp(7.0, 11.0);
                    let mark_cx = shape.x + 10.0 + mark_r;
                    draw_logo(ctx, &self.fmt_logo, mark_cx, center_y, mark_r, s, compact_alpha)?;

                    let time_w = 54.0;
                    // The Pro island keeps its camera at the right end, so
                    // the clock moves in to make room for the lens.
                    let lens_pad = if p.mode == crate::state::SurfaceMode::IslandPro { 18.0 } else { 0.0 };
                    let time_brush = solid(ctx, &primary)?;
                    time_brush.SetOpacity(compact_alpha);
                    draw_text(
                        ctx,
                        &p.time,
                        &self.fmt_pill_time,
                        D2D_RECT_F {
                            left: (shape.right() - 14.0 - lens_pad - time_w) * s,
                            top: visible_top * s,
                            right: (shape.right() - 14.0 - lens_pad) * s,
                            bottom: shape.bottom() * s,
                        },
                        &time_brush,
                    );

                    let mut mid_left = mark_cx + mark_r + 8.0;
                    let mid_right = shape.right() - 14.0 - lens_pad - time_w - 6.0;
                    if lens_pad > 0.0 {
                        draw_lens(ctx, shape.right() - 17.0, center_y, s, compact_alpha)?;
                    }
                    if p.media.available && mid_right - mid_left > 40.0 {
                        let art_size = (mark_r * 2.0).min(20.0);
                        if let Some(art) = &self.art_bitmap {
                            draw_rounded_art(
                                ctx,
                                art,
                                scaled_rect(mid_left, center_y - art_size * 0.5, art_size, art_size, s),
                                5.0 * s,
                                compact_alpha,
                            )?;
                            mid_left += art_size + 6.0;
                        }
                        let title = solid(ctx, &primary)?;
                        title.SetOpacity(compact_alpha);
                        draw_text(
                            ctx,
                            &p.media.title,
                            &self.fmt_pill_title,
                            D2D_RECT_F {
                                left: mid_left * s,
                                top: visible_top * s,
                                right: mid_right * s,
                                bottom: shape.bottom() * s,
                            },
                            &title,
                        );
                    }
                }
            }

            // Expanded content fades in.
            let panel_alpha = p.t.clamp(0.0, 1.0);
            if panel_alpha > 0.01 && p.layout == crate::state::Layout::Panel {
                let x = shape.x;
                let y = visible_top;
                let w = shape.w;
                let pad = 24.0;

                let big = solid(ctx, &primary)?;
                big.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    &p.time,
                    &self.fmt_big,
                    scaled_rect(x + pad, y + 12.0, w - pad * 2.0, 42.0, s),
                    &big,
                );

                let date_brush = solid(ctx, &secondary)?;
                date_brush.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    &p.date,
                    &self.fmt_label,
                    scaled_rect(x + pad, y + 52.0, w - pad * 2.0, 18.0, s),
                    &date_brush,
                );

                let media_left = x + w * 0.40;
                let media_w = w * 0.60 - pad;
                let has_lyrics = p.media.available && !p.lyrics.lines.is_empty();
                let label = if !p.media.available {
                    "NO MEDIA"
                } else if has_lyrics {
                    "LYRICS"
                } else if p.media.playing {
                    "NOW PLAYING"
                } else {
                    "PAUSED"
                };
                let label_brush = solid(ctx, &secondary)?;
                label_brush.SetOpacity(0.55 * panel_alpha);
                draw_text(
                    ctx,
                    label,
                    &self.fmt_wordmark,
                    scaled_rect(media_left, y + 14.0, media_w, 12.0, s),
                    &label_brush,
                );

                let mut text_left = media_left;
                let mut text_w = media_w;
                if let Some(art) = &self.art_bitmap {
                    let art_size = 40.0;
                    ctx.DrawBitmap(
                        art,
                        Some(&scaled_rect(media_left, y + 27.0, art_size, art_size, s)),
                        panel_alpha,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                    text_left += art_size + 12.0;
                    text_w -= art_size + 12.0;
                }

                if has_lyrics {
                    if let Some(current) = p.lyrics.index_at(p.media.position) {
                        for offset in [-1i32, 0, 1] {
                            let index = current as i32 + offset;
                            if index < 0 || index as usize >= p.lyrics.lines.len() {
                                continue;
                            }
                            let line = &p.lyrics.lines[index as usize];
                            let row_y = y + 28.0 + (offset + 1) as f32 * 16.0;
                            let brush = if offset == 0 {
                                let brush = solid(ctx, &primary)?;
                                brush.SetOpacity(panel_alpha);
                                brush
                            } else {
                                let brush = solid(ctx, &secondary)?;
                                brush.SetOpacity(0.4 * panel_alpha);
                                brush
                            };
                            draw_text(
                                ctx,
                                &line.text,
                                if offset == 0 {
                                    &self.fmt_body
                                } else {
                                    &self.fmt_small
                                },
                                scaled_rect(text_left, row_y, text_w, 16.0, s),
                                &brush,
                            );
                        }
                    }
                } else if p.media.available {
                    let title_brush = solid(ctx, &primary)?;
                    title_brush.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        &p.media.title,
                        &self.fmt_media_title,
                        scaled_rect(text_left, y + 28.0, text_w, 18.0, s),
                        &title_brush,
                    );

                    let artist_brush = solid(ctx, &secondary)?;
                    artist_brush.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        &p.media.artist,
                        &self.fmt_media_artist,
                        scaled_rect(text_left, y + 48.0, text_w, 16.0, s),
                        &artist_brush,
                    );
                }

                if p.page == 0 {
                    let divider = solid(ctx, &border)?;
                divider.SetOpacity(panel_alpha);
                ctx.FillRectangle(
                    &D2D_RECT_F {
                        left: (x + pad) * s,
                        top: (y + 80.0) * s,
                        right: (x + w - pad) * s,
                        bottom: (y + 81.0) * s,
                    },
                    &divider,
                );

                let bar_left = x + pad;
                let bar_w = w - pad * 2.0;
                let ratio = if p.media.duration > 0.0 {
                    (p.media.position / p.media.duration).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                if p.media.available {
                    draw_slider(ctx, bar_left, y + 91.5, bar_w, ratio, true, s, panel_alpha)?;
                } else {
                    draw_slider(ctx, bar_left, y + 91.5, bar_w, 0.0, false, s, panel_alpha)?;
                }

                if p.media.available {
                    let time_brush = solid(ctx, &secondary)?;
                    time_brush.SetOpacity(0.8 * panel_alpha);
                    draw_text(
                        ctx,
                        &format_time(p.media.position),
                        &self.fmt_time_left,
                        scaled_rect(bar_left, y + 99.0, bar_w * 0.5, 14.0, s),
                        &time_brush,
                    );
                    draw_text(
                        ctx,
                        &format_time(p.media.duration),
                        &self.fmt_time_right,
                        scaled_rect(bar_left + bar_w * 0.5, y + 99.0, bar_w * 0.5, 14.0, s),
                        &time_brush,
                    );

                    let transport = crate::state::transport(&p.shape);
                    let glyph_brush = solid(ctx, &secondary)?;
                    glyph_brush.SetOpacity(0.85 * panel_alpha);
                    draw_text(
                        ctx,
                        "\u{E892}",
                        &self.fmt_icon,
                        scaled_rect(
                            transport.prev.x,
                            transport.prev.y,
                            transport.prev.w,
                            transport.prev.h,
                            s,
                        ),
                        &glyph_brush,
                    );

                    let play_disc = violet_gradient(
                        ctx,
                        &scaled_rect(transport.play.x, transport.play.y, transport.play.w, transport.play.h, s),
                        panel_alpha,
                    )?;
                    ctx.FillEllipse(
                        &circle(
                            (transport.play.x + transport.play.w * 0.5) * s,
                            (transport.play.y + transport.play.h * 0.5) * s,
                            transport.play.w * 0.5 * s,
                        ),
                        &play_disc,
                    );
                    let play_brush = solid(ctx, &theme::rgb(0xFFFFFF, 1.0))?;
                    play_brush.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        if p.media.playing { "\u{E769}" } else { "\u{E768}" },
                        &self.fmt_icon,
                        scaled_rect(
                            transport.play.x,
                            transport.play.y,
                            transport.play.w,
                            transport.play.h,
                            s,
                        ),
                        &play_brush,
                    );

                    draw_text(
                        ctx,
                        "\u{E893}",
                        &self.fmt_icon,
                        scaled_rect(
                            transport.next.x,
                            transport.next.y,
                            transport.next.w,
                            transport.next.h,
                            s,
                        ),
                        &glyph_brush,
                    );
                }

                let divider2 = solid(ctx, &border)?;
                divider2.SetOpacity(panel_alpha);
                ctx.FillRectangle(
                    &D2D_RECT_F {
                        left: (x + pad) * s,
                        top: (y + 164.0) * s,
                        right: (x + w - pad) * s,
                        bottom: (y + 165.0) * s,
                    },
                    &divider2,
                );

                metric(
                    ctx,
                    "CPU",
                    p.cpu,
                    x + pad,
                    y + 172.0,
                    w - pad * 2.0,
                    s,
                    panel_alpha,
                    &primary,
                    &secondary,
                    &accent,
                    &track,
                    &self.fmt_label,
                    &self.fmt_value,
                );

                metric(
                    ctx,
                    "MEMORY",
                    p.ram,
                    x + pad,
                    y + 200.0,
                    w - pad * 2.0,
                    s,
                    panel_alpha,
                    &primary,
                    &secondary,
                    &accent,
                    &track,
                    &self.fmt_label,
                    &self.fmt_value,
                );

                }

                if p.page == 1 {
                let ctrl_y = 96.0;

                let divider3 = solid(ctx, &border)?;
                divider3.SetOpacity(panel_alpha);
                ctx.FillRectangle(
                    &D2D_RECT_F {
                        left: (x + pad) * s,
                        top: (y + ctrl_y - 8.0) * s,
                        right: (x + w - pad) * s,
                        bottom: (y + ctrl_y - 7.0) * s,
                    },
                    &divider3,
                );

                let label_brush = solid(ctx, &secondary)?;
                label_brush.SetOpacity(0.75 * panel_alpha);
                draw_text(
                    ctx,
                    "VOLUME",
                    &self.fmt_label,
                    scaled_rect(x + pad, y + ctrl_y, 120.0, 14.0, s),
                    &label_brush,
                );

                let value_text = if p.muted {
                    "MUTED".to_string()
                } else {
                    format!("{:.0}%", p.volume * 100.0)
                };
                let value_brush = solid(ctx, &primary)?;
                value_brush.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    &value_text,
                    &self.fmt_value,
                    scaled_rect(x + w - pad - 120.0, y + ctrl_y, 120.0, 14.0, s),
                    &value_brush,
                );

                let bar_w = w - pad * 2.0;
                let track_brush = solid(ctx, &track)?;
                track_brush.SetOpacity(panel_alpha);
                ctx.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: scaled_rect_raw(x + pad, y + ctrl_y + 18.0, bar_w, 3.0, s),
                        radiusX: 1.5 * s,
                        radiusY: 1.5 * s,
                    },
                    &track_brush,
                );
                if p.volume > 0.001 && !p.muted {
                    let fill_brush = solid(ctx, &accent)?;
                    fill_brush.SetOpacity(panel_alpha);
                    ctx.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: scaled_rect_raw(
                                x + pad,
                                y + ctrl_y + 18.0,
                                bar_w * p.volume.clamp(0.0, 1.0),
                                3.0,
                                s,
                            ),
                            radiusX: 1.5 * s,
                            radiusY: 1.5 * s,
                        },
                        &fill_brush,
                    );
                }

                let bright_label = solid(ctx, &secondary)?;
                bright_label.SetOpacity(0.75 * panel_alpha);
                draw_text(
                    ctx,
                    "BRIGHTNESS",
                    &self.fmt_label,
                    scaled_rect(x + pad, y + ctrl_y + 34.0, 160.0, 14.0, s),
                    &bright_label,
                );

                let bright_text = if p.has_brightness {
                    format!("{:.0}%", p.brightness * 100.0)
                } else {
                    "N/A".to_string()
                };
                let bright_value = solid(ctx, &primary)?;
                bright_value.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    &bright_text,
                    &self.fmt_value,
                    scaled_rect(x + w - pad - 120.0, y + ctrl_y + 34.0, 120.0, 14.0, s),
                    &bright_value,
                );

                let bright_track = solid(ctx, &track)?;
                bright_track.SetOpacity(panel_alpha);
                ctx.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: scaled_rect_raw(x + pad, y + ctrl_y + 52.0, bar_w, 3.0, s),
                        radiusX: 1.5 * s,
                        radiusY: 1.5 * s,
                    },
                    &bright_track,
                );
                if p.has_brightness && p.brightness > 0.001 {
                    let bright_fill = solid(ctx, &accent)?;
                    bright_fill.SetOpacity(panel_alpha);
                    ctx.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: scaled_rect_raw(
                                x + pad,
                                y + ctrl_y + 52.0,
                                bar_w * p.brightness.clamp(0.0, 1.0),
                                3.0,
                                s,
                            ),
                            radiusX: 1.5 * s,
                            radiusY: 1.5 * s,
                        },
                        &bright_fill,
                    );
                }

                let (wifi_text, wifi_on) = match p.net.wifi {
                    Some(true) => ("WI-FI ON", true),
                    Some(false) => ("WI-FI OFF", false),
                    None => ("WI-FI N/A", false),
                };
                let (bt_text, bt_on) = match p.net.bluetooth {
                    Some(true) => ("BLUETOOTH ON", true),
                    Some(false) => ("BLUETOOTH OFF", false),
                    None => ("BLUETOOTH N/A", false),
                };

                let wifi_brush = if wifi_on {
                    solid(ctx, &theme::rgb(theme::color::SUCCESS, panel_alpha))?
                } else {
                    let brush = solid(ctx, &secondary)?;
                    brush.SetOpacity(0.6 * panel_alpha);
                    brush
                };
                draw_text(
                    ctx,
                    wifi_text,
                    &self.fmt_status,
                    scaled_rect(x + pad, y + ctrl_y + 74.0, 140.0, 14.0, s),
                    &wifi_brush,
                );

                let bt_brush = if bt_on {
                    solid(ctx, &theme::rgb(theme::color::SUCCESS, panel_alpha))?
                } else {
                    let brush = solid(ctx, &secondary)?;
                    brush.SetOpacity(0.6 * panel_alpha);
                    brush
                };
                draw_text(
                    ctx,
                    bt_text,
                    &self.fmt_status,
                    scaled_rect(x + pad + 150.0, y + ctrl_y + 74.0, 180.0, 14.0, s),
                    &bt_brush,
                );
                }

                if p.page == 2 {
                    let focus_label = solid(ctx, &secondary)?;
                    focus_label.SetOpacity(0.7 * panel_alpha);
                    draw_text(
                        ctx,
                        "FOCUS SESSION",
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 96.0, 200.0, 14.0, s),
                        &focus_label,
                    );

                    let focus_brush = solid(ctx, &primary)?;
                    focus_brush.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        &format_time(p.focus_remaining),
                        &self.fmt_focus,
                        scaled_rect(x + 40.0, y + 112.0, w - 80.0, 64.0, s),
                        &focus_brush,
                    );

                    let focus_state = if p.focus_running { "RUNNING" } else { "PAUSED" };
                    let state_brush = solid(ctx, &secondary)?;
                    state_brush.SetOpacity(0.6 * panel_alpha);
                    draw_text(
                        ctx,
                        focus_state,
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 184.0, 200.0, 14.0, s),
                        &state_brush,
                    );

                    let reset_brush = solid(ctx, &secondary)?;
                    reset_brush.SetOpacity(0.45 * panel_alpha);
                    draw_text(
                        ctx,
                        "RESET",
                        &self.fmt_tab,
                        scaled_rect(x + w * 0.5 - 50.0, y + 214.0, 100.0, 24.0, s),
                        &reset_brush,
                    );
                }

                if p.page == 3 {
                    let head = solid(ctx, &secondary)?;
                    head.SetOpacity(0.6 * panel_alpha);
                    draw_text(
                        ctx,
                        "TASKS",
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 94.0, 160.0, 14.0, s),
                        &head,
                    );

                    let open = p.tasks.iter().filter(|t| !t.done).count();
                    let count = solid(ctx, &secondary)?;
                    count.SetOpacity(0.45 * panel_alpha);
                    draw_text(
                        ctx,
                        &format!("{open} OPEN"),
                        &self.fmt_wordmark,
                        scaled_rect(x + w - pad - 120.0, y + 94.0, 120.0, 14.0, s),
                        &count,
                    );

                    if p.tasks.is_empty() {
                        let empty = solid(ctx, &secondary)?;
                        empty.SetOpacity(0.35 * panel_alpha);
                        draw_text(
                            ctx,
                            "No tasks yet. Add one below.",
                            &self.fmt_body,
                            scaled_rect(x + pad, y + 118.0, w - pad * 2.0, 18.0, s),
                            &empty,
                        );
                    }

                    for index in 0..4 {
                        let Some(task) = p.tasks.get(index) else {
                            break;
                        };
                        let row = crate::state::task_row(&p.shape, index);
                        let box_rect = scaled_rect(row.x, row.y + 4.0, 12.0, 12.0, s);
                        if task.done {
                            let fill = solid(ctx, &accent)?;
                            fill.SetOpacity(0.85 * panel_alpha);
                            ctx.FillRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: box_rect,
                                    radiusX: 3.0 * s,
                                    radiusY: 3.0 * s,
                                },
                                &fill,
                            );
                            let check = solid(ctx, &theme::rgb(theme::color::GRAPHITE_950, 1.0))?;
                            check.SetOpacity(panel_alpha);
                            draw_text(
                                ctx,
                                "\u{2713}",
                                &self.fmt_tab,
                                scaled_rect(row.x, row.y + 4.0, 12.0, 12.0, s),
                                &check,
                            );
                        } else {
                            let outline = solid(ctx, &theme::rgb(theme::accent::c800(), 1.0))?;
                            outline.SetOpacity(0.9 * panel_alpha);
                            ctx.DrawRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: box_rect,
                                    radiusX: 3.0 * s,
                                    radiusY: 3.0 * s,
                                },
                                &outline,
                                1.0,
                                None,
                            );
                        }

                        let text_brush = solid(ctx, &primary)?;
                        text_brush.SetOpacity(if task.done { 0.35 } else { 0.92 } * panel_alpha);
                        draw_text(
                            ctx,
                            &task.text,
                            &self.fmt_body,
                            scaled_rect(row.x + 20.0, row.y, row.w - 20.0, row.h, s),
                            &text_brush,
                        );
                    }

                    let add_rect = crate::state::task_add(&p.shape);
                    if p.editing {
                        let border_brush = solid(ctx, &accent)?;
                        border_brush.SetOpacity(0.8 * panel_alpha);
                        ctx.DrawRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: scaled_rect(add_rect.x - 6.0, add_rect.y, add_rect.w + 12.0, add_rect.h, s),
                                radiusX: 6.0 * s,
                                radiusY: 6.0 * s,
                            },
                            &border_brush,
                            1.0,
                            None,
                        );
                        let text_brush = solid(ctx, &primary)?;
                        text_brush.SetOpacity(panel_alpha);
                        draw_text(
                            ctx,
                            &format!("{}\u{258F}", p.edit_text),
                            &self.fmt_body,
                            scaled_rect(add_rect.x, add_rect.y, add_rect.w, add_rect.h, s),
                            &text_brush,
                        );
                    } else {
                        let add_brush = solid(ctx, &secondary)?;
                        add_brush.SetOpacity(0.5 * panel_alpha);
                        draw_text(
                            ctx,
                            "+  ADD TASK",
                            &self.fmt_body,
                            scaled_rect(add_rect.x, add_rect.y, add_rect.w, add_rect.h, s),
                            &add_brush,
                        );
                    }

                    let water_label = solid(ctx, &secondary)?;
                    water_label.SetOpacity(0.6 * panel_alpha);
                    draw_text(
                        ctx,
                        "WATER",
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 230.0, 80.0, 14.0, s),
                        &water_label,
                    );

                    for index in 0..8 {
                        let dot = crate::state::water_dot(&p.shape, index);
                        let cx = (dot.x + dot.w * 0.5) * s;
                        let cy = (dot.y + dot.h * 0.5) * s;
                        let r = 3.5 * s;
                        let brush = if index < p.water as usize {
                            solid(ctx, &accent)?
                        } else {
                            solid(ctx, &track)?
                        };
                        brush.SetOpacity(0.95 * panel_alpha);
                        ctx.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: cx - r,
                                    top: cy - r,
                                    right: cx + r,
                                    bottom: cy + r,
                                },
                                radiusX: r,
                                radiusY: r,
                            },
                            &brush,
                        );
                    }

                    let water_count = solid(ctx, &primary)?;
                    water_count.SetOpacity(0.85 * panel_alpha);
                    draw_text(
                        ctx,
                        &format!("{}/{}", p.water, p.water_goal),
                        &self.fmt_value,
                        scaled_rect(x + w - pad - 60.0, y + 230.0, 60.0, 14.0, s),
                        &water_count,
                    );
                }

                if p.page == 4 {
                    let head = solid(ctx, &secondary)?;
                    head.SetOpacity(0.6 * panel_alpha);
                    draw_text(
                        ctx,
                        "SHELF",
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 94.0, 200.0, 14.0, s),
                        &head,
                    );

                    let clear = solid(ctx, &secondary)?;
                    clear.SetOpacity(0.4 * panel_alpha);
                    let clear_rect = crate::state::clip_clear(&p.shape);
                    draw_text(
                        ctx,
                        "CLEAR",
                        &self.fmt_tab,
                        scaled_rect(clear_rect.x, clear_rect.y, clear_rect.w, clear_rect.h, s),
                        &clear,
                    );

                    if p.shelf_paths.is_empty() {
                        let empty = solid(ctx, &secondary)?;
                        empty.SetOpacity(0.35 * panel_alpha);
                        draw_text(
                            ctx,
                            "Drag files onto the notch.",
                            &self.fmt_body,
                            scaled_rect(x + pad, y + 118.0, w - pad * 2.0, 18.0, s),
                            &empty,
                        );
                    }

                    for (index, path) in p.shelf_paths.iter().take(5).enumerate() {
                        let row = crate::state::clip_row(&p.shape, index);
                        let name = std::path::Path::new(path)
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.clone());
                        let brush = solid(ctx, &primary)?;
                        brush.SetOpacity(0.88 * panel_alpha);
                        draw_text(
                            ctx,
                            &name,
                            &self.fmt_body,
                            scaled_rect(row.x, row.y, row.w - 48.0, row.h, s),
                            &brush,
                        );
                        let action = solid(ctx, &secondary)?;
                        action.SetOpacity(0.3 * panel_alpha);
                        draw_text(
                            ctx,
                            "OPEN",
                            &self.fmt_tab,
                            scaled_rect(row.x + row.w - 44.0, row.y, 44.0, row.h, s),
                            &action,
                        );

                        if index < 4 {
                            let divider = solid(ctx, &border)?;
                            divider.SetOpacity(0.5 * panel_alpha);
                            ctx.FillRectangle(
                                &D2D_RECT_F {
                                    left: row.x * s,
                                    top: (row.y + row.h) * s,
                                    right: (row.x + row.w) * s,
                                    bottom: (row.y + row.h + 1.0) * s,
                                },
                                &divider,
                            );
                        }
                    }
                }

                if p.page == 5 {
                    let head = solid(ctx, &secondary)?;
                    head.SetOpacity(0.6 * panel_alpha);
                    draw_text(
                        ctx,
                        "CLIPBOARD",
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 94.0, 200.0, 14.0, s),
                        &head,
                    );

                    let clear = solid(ctx, &secondary)?;
                    clear.SetOpacity(0.4 * panel_alpha);
                    let clear_rect = crate::state::clip_clear(&p.shape);
                    draw_text(
                        ctx,
                        "CLEAR",
                        &self.fmt_tab,
                        scaled_rect(clear_rect.x, clear_rect.y, clear_rect.w, clear_rect.h, s),
                        &clear,
                    );

                    if p.clip_items.is_empty() {
                        let empty = solid(ctx, &secondary)?;
                        empty.SetOpacity(0.35 * panel_alpha);
                        draw_text(
                            ctx,
                            "Copy text anywhere. It shows up here.",
                            &self.fmt_body,
                            scaled_rect(x + pad, y + 118.0, w - pad * 2.0, 18.0, s),
                            &empty,
                        );
                    }

                    for (index, text) in p.clip_items.iter().take(5).enumerate() {
                        let row = crate::state::clip_row(&p.shape, index);
                        let number = solid(ctx, &secondary)?;
                        number.SetOpacity(0.3 * panel_alpha);
                        draw_text(
                            ctx,
                            &format!("{:02}", index + 1),
                            &self.fmt_time_left,
                            scaled_rect(row.x, row.y, 22.0, row.h, s),
                            &number,
                        );

                        let preview: String = text
                            .replace(['\r', '\n', '\t'], " ")
                            .chars()
                            .take(56)
                            .collect();
                        let brush = solid(ctx, &primary)?;
                        brush.SetOpacity(0.85 * panel_alpha);
                        draw_text(
                            ctx,
                            &preview,
                            &self.fmt_small,
                            scaled_rect(row.x + 24.0, row.y, row.w - 24.0, row.h, s),
                            &brush,
                        );

                        if index < 4 {
                            let divider = solid(ctx, &border)?;
                            divider.SetOpacity(0.5 * panel_alpha);
                            ctx.FillRectangle(
                                &D2D_RECT_F {
                                    left: (row.x + 24.0) * s,
                                    top: (row.y + row.h) * s,
                                    right: (row.x + row.w) * s,
                                    bottom: (row.y + row.h + 1.0) * s,
                                },
                                &divider,
                            );
                        }
                    }
                }

                if p.page == 6 {
                    let head = solid(ctx, &secondary)?;
                    head.SetOpacity(0.6 * panel_alpha);
                    draw_text(
                        ctx,
                        "APPS",
                        &self.fmt_status,
                        scaled_rect(x + pad, y + 94.0, 160.0, 14.0, s),
                        &head,
                    );

                    let count = solid(ctx, &secondary)?;
                    count.SetOpacity(0.45 * panel_alpha);
                    draw_text(
                        ctx,
                        &format!("{} APPS", p.apps.len()),
                        &self.fmt_wordmark,
                        scaled_rect(x + w - pad - 120.0, y + 94.0, 120.0, 14.0, s),
                        &count,
                    );

                    if p.apps.is_empty() {
                        let empty = solid(ctx, &secondary)?;
                        empty.SetOpacity(0.35 * panel_alpha);
                        draw_text(
                            ctx,
                            "No pinned apps found. Pin apps to the taskbar.",
                            &self.fmt_body,
                            scaled_rect(x + pad, y + 118.0, w - pad * 2.0, 18.0, s),
                            &empty,
                        );
                    }

                    for (index, name) in p.apps.iter().take(5).enumerate() {
                        let row = crate::state::clip_row(&p.shape, index);
                        let brush = solid(ctx, &primary)?;
                        brush.SetOpacity(0.88 * panel_alpha);
                        draw_text(
                            ctx,
                            name,
                            &self.fmt_body,
                            scaled_rect(row.x, row.y, row.w - 48.0, row.h, s),
                            &brush,
                        );
                        let action = solid(ctx, &secondary)?;
                        action.SetOpacity(0.3 * panel_alpha);
                        draw_text(
                            ctx,
                            "OPEN",
                            &self.fmt_tab,
                            scaled_rect(row.x + row.w - 44.0, row.y, 44.0, row.h, s),
                            &action,
                        );
                        if index < 4 {
                            let divider = solid(ctx, &border)?;
                            divider.SetOpacity(0.5 * panel_alpha);
                            ctx.FillRectangle(
                                &D2D_RECT_F {
                                    left: row.x * s,
                                    top: (row.y + row.h) * s,
                                    right: (row.x + row.w) * s,
                                    bottom: (row.y + row.h + 1.0) * s,
                                },
                                &divider,
                            );
                        }
                    }
                }

                let tabs = crate::state::tabs(&p.shape);
                let tab_active = solid(ctx, &primary)?;
                tab_active.SetOpacity(panel_alpha);
                let tab_idle = solid(ctx, &secondary)?;
                tab_idle.SetOpacity(0.45 * panel_alpha);
                draw_text(
                    ctx,
                    "OVERVIEW",
                    &self.fmt_tab,
                    scaled_rect(tabs.overview.x, tabs.overview.y, tabs.overview.w, tabs.overview.h, s),
                    if p.page == 0 { &tab_active } else { &tab_idle },
                );
                draw_text(
                    ctx,
                    "CONTROLS",
                    &self.fmt_tab,
                    scaled_rect(tabs.controls.x, tabs.controls.y, tabs.controls.w, tabs.controls.h, s),
                    if p.page == 1 { &tab_active } else { &tab_idle },
                );
                draw_text(
                    ctx,
                    "FOCUS",
                    &self.fmt_tab,
                    scaled_rect(tabs.focus.x, tabs.focus.y, tabs.focus.w, tabs.focus.h, s),
                    if p.page == 2 { &tab_active } else { &tab_idle },
                );
                draw_text(
                    ctx,
                    "TASKS",
                    &self.fmt_tab,
                    scaled_rect(tabs.tasks.x, tabs.tasks.y, tabs.tasks.w, tabs.tasks.h, s),
                    if p.page == 3 { &tab_active } else { &tab_idle },
                );
                draw_text(
                    ctx,
                    "SHELF",
                    &self.fmt_tab,
                    scaled_rect(tabs.shelf.x, tabs.shelf.y, tabs.shelf.w, tabs.shelf.h, s),
                    if p.page == 4 { &tab_active } else { &tab_idle },
                );
                draw_text(
                    ctx,
                    "CLIP",
                    &self.fmt_tab,
                    scaled_rect(tabs.clip.x, tabs.clip.y, tabs.clip.w, tabs.clip.h, s),
                    if p.page == 5 { &tab_active } else { &tab_idle },
                );
                draw_text(
                    ctx,
                    "APPS",
                    &self.fmt_tab,
                    scaled_rect(tabs.apps.x, tabs.apps.y, tabs.apps.w, tabs.apps.h, s),
                    if p.page == 6 { &tab_active } else { &tab_idle },
                );

                let underline = if p.page == 0 {
                    &tabs.overview
                } else if p.page == 1 {
                    &tabs.controls
                } else if p.page == 2 {
                    &tabs.focus
                } else if p.page == 3 {
                    &tabs.tasks
                } else if p.page == 4 {
                    &tabs.shelf
                } else if p.page == 5 {
                    &tabs.clip
                } else {
                    &tabs.apps
                };
                let line = solid(ctx, &accent)?;
                line.SetOpacity(panel_alpha);
                ctx.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: scaled_rect_raw(
                            underline.x + 22.0,
                            underline.y + 20.0,
                            underline.w - 44.0,
                            2.0,
                            s,
                        ),
                        radiusX: s,
                        radiusY: s,
                    },
                    &line,
                );
            }

            if panel_alpha > 0.01 && p.layout == crate::state::Layout::Wide {
                let x = shape.x;
                let y = visible_top;
                let w = shape.w;
                let media_left = x + 84.0;

                let label = if !p.media.available {
                    "NO MEDIA"
                } else if p.media.playing {
                    "NOW PLAYING"
                } else {
                    "PAUSED"
                };
                let label_brush = solid(ctx, &secondary)?;
                label_brush.SetOpacity(0.5 * panel_alpha);
                draw_text(
                    ctx,
                    label,
                    &self.fmt_status,
                    scaled_rect(media_left, y + 12.0, 200.0, 12.0, s),
                    &label_brush,
                );

                if let Some(art) = &self.art_bitmap {
                    draw_rounded_art(
                        ctx,
                        art,
                        scaled_rect(x + 22.0, y + 24.0, 48.0, 48.0, s),
                        12.0 * s,
                        panel_alpha,
                    )?;
                } else {
                    draw_glass(
                        ctx,
                        scaled_rect(x + 22.0, y + 24.0, 48.0, 48.0, s),
                        12.0 * s,
                        panel_alpha,
                        Glass::Tile,
                        s,
                    )?;
                    draw_logo(ctx, &self.fmt_logo, x + 46.0, y + 48.0, 13.0, s, panel_alpha)?;
                }

                if p.media.available {
                    let title_brush = solid(ctx, &primary)?;
                    title_brush.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        &p.media.title,
                        &self.fmt_wide_title,
                        scaled_rect(media_left, y + 22.0, 262.0, 18.0, s),
                        &title_brush,
                    );

                    let has_lyrics = !p.lyrics.lines.is_empty();
                    let sub = if has_lyrics {
                        p.lyrics
                            .index_at(p.media.position)
                            .and_then(|index| p.lyrics.lines.get(index))
                            .map(|line| line.text.clone())
                            .unwrap_or_default()
                    } else {
                        p.media.artist.clone()
                    };
                    let sub_brush = solid(ctx, if has_lyrics { &accent } else { &secondary })?;
                    sub_brush.SetOpacity(0.85 * panel_alpha);
                    draw_text(
                        ctx,
                        &sub,
                        &self.fmt_wide_sub,
                        scaled_rect(media_left, y + 44.0, 262.0, 16.0, s),
                        &sub_brush,
                    );
                }

                let bar_w = 276.0;
                let ratio = if p.media.duration > 0.0 {
                    (p.media.position / p.media.duration).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                draw_slider(ctx, media_left, y + 73.5, bar_w, ratio, true, s, panel_alpha)?;
                let time_brush = solid(ctx, &secondary)?;
                time_brush.SetOpacity(0.65 * panel_alpha);
                draw_text(
                    ctx,
                    &format_time(p.media.position),
                    &self.fmt_time_left,
                    scaled_rect(media_left, y + 80.0, 100.0, 14.0, s),
                    &time_brush,
                );
                draw_text(
                    ctx,
                    &format_time(p.media.duration),
                    &self.fmt_time_right,
                    scaled_rect(media_left + bar_w - 100.0, y + 80.0, 100.0, 14.0, s),
                    &time_brush,
                );

                let wide = crate::state::wide_layout(&p.shape);
                let glyph_brush = solid(ctx, &secondary)?;
                glyph_brush.SetOpacity(0.85 * panel_alpha);
                draw_text(
                    ctx,
                    "\u{E892}",
                    &self.fmt_icon,
                    scaled_rect(wide.prev.x, wide.prev.y, wide.prev.w, wide.prev.h, s),
                    &glyph_brush,
                );
                let play_disc = violet_gradient(
                    ctx,
                    &scaled_rect(wide.play.x, wide.play.y, wide.play.w, wide.play.h, s),
                    panel_alpha,
                )?;
                ctx.FillEllipse(
                    &circle(
                        (wide.play.x + wide.play.w * 0.5) * s,
                        (wide.play.y + wide.play.h * 0.5) * s,
                        wide.play.w * 0.5 * s,
                    ),
                    &play_disc,
                );
                let play_brush = solid(ctx, &theme::rgb(0xFFFFFF, 1.0))?;
                play_brush.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    if p.media.playing { "\u{E769}" } else { "\u{E768}" },
                    &self.fmt_icon,
                    scaled_rect(wide.play.x, wide.play.y, wide.play.w, wide.play.h, s),
                    &play_brush,
                );
                draw_text(
                    ctx,
                    "\u{E893}",
                    &self.fmt_icon,
                    scaled_rect(wide.next.x, wide.next.y, wide.next.w, wide.next.h, s),
                    &glyph_brush,
                );

                let divider = solid(ctx, &border)?;
                divider.SetOpacity(0.8 * panel_alpha);
                ctx.FillRectangle(
                    &D2D_RECT_F {
                        left: (x + 560.0) * s,
                        top: (y + 22.0) * s,
                        right: (x + 561.0) * s,
                        bottom: (y + 118.0) * s,
                    },
                    &divider,
                );

                let metrics = [
                    ("CPU", p.cpu.clamp(0.0, 100.0), format!("{:.0}%", p.cpu)),
                    ("RAM", p.ram.clamp(0.0, 100.0), format!("{:.0}%", p.ram)),
                    (
                        if p.charging { "POWER" } else { "BATTERY" },
                        p.battery.map(|b| b as f32).unwrap_or(0.0),
                        p.battery
                            .map(|b| format!("{b}%"))
                            .unwrap_or_else(|| "N/A".to_string()),
                    ),
                ];
                for (index, (label, value, text)) in metrics.iter().enumerate() {
                    let cx = x + 576.0 + index as f32 * 64.0 + 29.0;
                    let cy = y + 54.0;
                    let ring_color = if index == 2 {
                        if *value <= 15.0 {
                            theme::rgb(theme::color::DANGER, 1.0)
                        } else {
                            theme::rgb(theme::accent::battery(), 1.0)
                        }
                    } else if *value >= theme::metric::DANGER_THRESHOLD {
                        theme::rgb(theme::color::DANGER, 1.0)
                    } else if *value >= theme::metric::WARNING_THRESHOLD {
                        theme::rgb(theme::color::WARNING, 1.0)
                    } else {
                        accent
                    };
                    draw_ring(ctx, cx, cy, 22.0, 4.0, value / 100.0, &ring_color, s, panel_alpha)?;
                    let m_value = solid(ctx, &primary)?;
                    m_value.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        text,
                        &self.fmt_clock,
                        scaled_rect(cx - 22.0, cy - 10.0, 44.0, 20.0, s),
                        &m_value,
                    );
                    let m_label = solid(ctx, &secondary)?;
                    m_label.SetOpacity(0.7 * panel_alpha);
                    draw_text(
                        ctx,
                        label,
                        &self.fmt_tab,
                        scaled_rect(cx - 30.0, y + 84.0, 60.0, 14.0, s),
                        &m_label,
                    );
                }

                let clock_brush = solid(ctx, &primary)?;
                clock_brush.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    &p.time,
                    &self.fmt_wide_time,
                    scaled_rect(x + w - 122.0, y + 20.0, 100.0, 28.0, s),
                    &clock_brush,
                );
                let date_brush = solid(ctx, &secondary)?;
                date_brush.SetOpacity(0.55 * panel_alpha);
                draw_text(
                    ctx,
                    &p.date,
                    &self.fmt_wordmark,
                    scaled_rect(x + w - 122.0, y + 50.0, 100.0, 12.0, s),
                    &date_brush,
                );
                let hint_brush = solid(ctx, &secondary)?;
                hint_brush.SetOpacity(0.3 * panel_alpha);
                draw_text(
                    ctx,
                    "OPEN  \u{25BE}",
                    &self.fmt_tab,
                    scaled_rect(x + w - 122.0, y + 92.0, 100.0, 16.0, s),
                    &hint_brush,
                );
            }

            if panel_alpha > 0.01 && p.layout == crate::state::Layout::Hub {
                let top = visible_top;
                let cy = (top + shape.bottom()) * 0.5;
                let x = shape.x;
                let w = shape.w;
                draw_logo(ctx, &self.fmt_logo, x + 30.0, cy, 13.0, s, panel_alpha)?;
                let mut left = x + 54.0;
                if let Some(art) = &self.art_bitmap {
                    draw_rounded_art(ctx, art, scaled_rect(left, cy - 14.0, 28.0, 28.0, s), 7.0 * s, panel_alpha)?;
                    left += 36.0;
                }
                let title = solid(ctx, &primary)?;
                title.SetOpacity(panel_alpha);
                let sub = solid(ctx, &secondary)?;
                sub.SetOpacity(0.8 * panel_alpha);
                if p.media.available {
                    draw_text(ctx, &p.media.title, &self.fmt_pill_title, scaled_rect(left, cy - 15.0, 170.0, 16.0, s), &title);
                    draw_text(ctx, &p.media.artist, &self.fmt_small, scaled_rect(left, cy + 1.0, 170.0, 14.0, s), &sub);
                } else {
                    draw_text(ctx, "NOTCH", &self.fmt_pill_title, scaled_rect(left, cy - 8.0, 170.0, 16.0, s), &title);
                }

                let line = solid(ctx, &theme::rgb(theme::accent::light(), 0.10 * panel_alpha))?;
                let sys_x = x + 262.0;
                ctx.FillRectangle(&scaled_rect(sys_x - 12.0, cy - 13.0, 1.0, 26.0, s), &line);
                for (index, (label, value)) in [("CPU", p.cpu), ("RAM", p.ram)].iter().enumerate() {
                    let col = sys_x + index as f32 * 48.0;
                    draw_text(ctx, label, &self.fmt_status, scaled_rect(col, cy - 14.0, 44.0, 12.0, s), &sub);
                    draw_text(ctx, &format!("{value:.0}%"), &self.fmt_pill_title, scaled_rect(col, cy - 2.0, 44.0, 16.0, s), &title);
                }
                ctx.FillRectangle(&scaled_rect(sys_x + 98.0, cy - 13.0, 1.0, 26.0, s), &line);
                let on = |state: Option<bool>| -> Result<ID2D1SolidColorBrush> {
                    let color = if state == Some(true) {
                        theme::rgb(theme::accent::text1(), panel_alpha)
                    } else {
                        theme::rgb(theme::accent::disabled(), panel_alpha)
                    };
                    solid(ctx, &color)
                };
                draw_text(ctx, "\u{E701}", &self.fmt_icon, scaled_rect(sys_x + 108.0, cy - 12.0, 24.0, 24.0, s), &on(p.net.wifi)?);
                draw_text(ctx, "\u{E702}", &self.fmt_icon, scaled_rect(sys_x + 136.0, cy - 12.0, 24.0, 24.0, s), &on(p.net.bluetooth)?);

                let right = x + w - 44.0;
                draw_text(ctx, &p.date, &self.fmt_time_right, scaled_rect(right - 120.0, cy - 15.0, 120.0, 14.0, s), &sub);
                draw_text(ctx, &p.time, &self.fmt_pill_time, scaled_rect(right - 120.0, cy - 1.0, 120.0, 16.0, s), &title);
                draw_text(ctx, "\u{E70E}", &self.fmt_icon_sm, scaled_rect(x + w - 38.0, cy - 10.0, 20.0, 20.0, s), &sub);
            }

            let ctx = &self.ctx;
            ctx.EndDraw(None, None)?;
            let hr = self.swapchain.Present(0, DXGI_PRESENT(0));
            hr.ok()?;
        }
        Ok(())
    }

    /// `hub`: Some(slide) when drawn inside the Hub overlay. The caller owns
    /// the card backgrounds and the fade layer; this shifts content into place.
    fn draw_cards(&self, p: &DrawParams, hub: Option<f32>) -> Result<()> {
        let s = p.scale;
        let hits = crate::state::card_hits();
        unsafe {
            let ctx = &self.ctx;
            let primary = solid(ctx, &theme::rgb(theme::accent::text1(), 1.0))?;
            let secondary = solid(ctx, &theme::rgb(theme::accent::text2(), 1.0))?;
            let accent = solid(ctx, &theme::rgb(theme::accent::c600(), 1.0))?;
            let border_color = theme::rgb(theme::accent::c800(), 0.32 * p.opacity);

            let shift = |offset: (f32, f32), slide: f32| windows_numerics::Matrix3x2 {
                M11: 1.0,
                M12: 0.0,
                M21: 0.0,
                M22: 1.0,
                M31: offset.0 * s,
                M32: (offset.1 + slide) * s,
            };
            if let Some(slide) = hub {
                ctx.SetTransform(&shift(crate::state::hub::quick_offset(), slide));
            }
            for card in [crate::state::quick_card(), crate::state::shelf_card()] {
                if hub.is_some() {
                    break;
                }
                let rect = scaled_rect(card.x, card.y, card.w, card.h, s);
                let rr = D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: 20.0 * s,
                    radiusY: 20.0 * s,
                };
                draw_glass(ctx, rr.rect, 20.0 * s, p.opacity, Glass::Card, s)?;
            }

            let pad = 32.0;
            draw_text(
                ctx,
                "Quick Controls",
                &self.fmt_card_title,
                scaled_rect(pad, 22.0, 220.0, 20.0, s),
                &primary,
            );

            // Wi-Fi / Bluetooth tiles with round icon badges.
            let wifi_state = match p.net.wifi {
                Some(true) => ("Connected", true),
                Some(false) => ("Off", false),
                None => ("N/A", false),
            };
            let bt_state = match p.net.bluetooth {
                Some(true) => ("On", true),
                Some(false) => ("Off", false),
                None => ("N/A", false),
            };
            for (index, (glyph, name, (state, on))) in [
                ("\u{E701}", "Wi-Fi", wifi_state),
                ("\u{E702}", "Bluetooth", bt_state),
            ]
            .into_iter()
            .enumerate()
            {
                let tx = pad + index as f32 * 184.0;
                draw_glass(ctx, scaled_rect(tx, 46.0, 176.0, 30.0, s), 10.0 * s, 1.0, Glass::Tile, s)?;
                draw_icon_badge(ctx, &self.fmt_icon_sm, glyph, tx + 16.0, 61.0, 11.0, on, s, 1.0)?;
                draw_text(ctx, name, &self.fmt_pill_title, scaled_rect(tx + 34.0, 46.0, 80.0, 30.0, s), &primary);
                let state_brush = solid(
                    ctx,
                    &theme::rgb(if on { theme::accent::c400() } else { theme::accent::muted() }, 1.0),
                )?;
                draw_text(ctx, state, &self.fmt_small, scaled_rect(tx + 108.0, 46.0, 64.0, 30.0, s), &state_brush);
            }

            draw_text(ctx, "\u{E767}", &self.fmt_icon_sm, scaled_rect(pad, 80.0, 16.0, 16.0, s), &secondary);
            draw_text(ctx, "Audio", &self.fmt_label, scaled_rect(pad + 22.0, 80.0, 160.0, 14.0, s), &primary);
            let volume_text = if p.muted {
                "Muted".to_string()
            } else {
                format!("{:.0}%", p.volume * 100.0)
            };
            draw_text(ctx, &volume_text, &self.fmt_value, scaled_rect(232.0, 80.0, 160.0, 14.0, s), &primary);
            let vol = if p.muted { 0.0 } else { p.volume };
            draw_slider(ctx, pad, 103.0, 360.0, vol, true, s, 1.0)?;

            draw_text(ctx, "\u{E706}", &self.fmt_icon_sm, scaled_rect(pad, 126.0, 16.0, 16.0, s), &secondary);
            draw_text(ctx, "Brightness", &self.fmt_label, scaled_rect(pad + 22.0, 126.0, 180.0, 14.0, s), &primary);
            let bright = if p.has_brightness {
                format!("{:.0}%", p.brightness * 100.0)
            } else {
                "N/A".to_string()
            };
            draw_text(ctx, &bright, &self.fmt_value, scaled_rect(232.0, 126.0, 160.0, 14.0, s), &primary);
            draw_slider(
                ctx,
                pad,
                149.0,
                360.0,
                if p.has_brightness { p.brightness } else { 0.0 },
                p.has_brightness,
                s,
                1.0,
            )?;

            let divider = solid(ctx, &border_color)?;
            ctx.FillRectangle(
                &D2D_RECT_F {
                    left: pad * s,
                    top: 176.0 * s,
                    right: 392.0 * s,
                    bottom: 177.0 * s,
                },
                &divider,
            );

            for (rect, glyph, label, active, glyph_color) in [
                (hits.focus, "\u{E7E8}", "Focus Mode", p.focus_running, theme::accent::c400()),
                (hits.battery, "\u{EBA6}", "Battery Saver", false, theme::color::SUCCESS),
                (hits.record, "\u{E7C8}", "Screen Record", false, theme::color::DANGER),
                (hits.theme, "\u{E790}", "Theme", false, theme::accent::c300()),
            ] {
                let rr = D2D1_ROUNDED_RECT {
                    rect: scaled_rect(rect.x, rect.y, rect.w, rect.h, s),
                    radiusX: 10.0 * s,
                    radiusY: 10.0 * s,
                };
                draw_glass(ctx, rr.rect, 10.0 * s, 1.0, Glass::Tile, s)?;
                if active {
                    let fill = solid(ctx, &theme::rgb(theme::accent::c600(), 0.22))?;
                    ctx.FillRoundedRectangle(&rr, &fill);
                    let rim = solid(ctx, &theme::rgb(theme::accent::c500(), 0.55))?;
                    ctx.DrawRoundedRectangle(&rr, &rim, 1.0, None);
                }
                let icon = solid(ctx, &theme::rgb(glyph_color, 1.0))?;
                draw_text(
                    ctx,
                    glyph,
                    &self.fmt_icon_sm,
                    scaled_rect(rect.x + 8.0, rect.y, 22.0, rect.h, s),
                    &icon,
                );
                let brush = if active { &accent } else { &primary };
                draw_text(
                    ctx,
                    label,
                    &self.fmt_pill_title,
                    scaled_rect(rect.x + 34.0, rect.y, rect.w - 40.0, rect.h, s),
                    brush,
                );
            }

            if let Some(slide) = hub {
                ctx.SetTransform(&shift(crate::state::hub::shelf_offset(), slide));
                let shelf = crate::state::shelf_card();
                let visible = crate::state::hub::shelf().h;
                ctx.PushAxisAlignedClip(
                    &scaled_rect(shelf.x, shelf.y, shelf.w, visible, s),
                    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                );
            }
            let folder = solid(ctx, &theme::rgb(theme::color::WARNING, 1.0))?;
            draw_text(ctx, "\u{E8B7}", &self.fmt_icon_sm, scaled_rect(pad, 366.0, 16.0, 20.0, s), &folder);
            draw_text(ctx, "File Shelf", &self.fmt_card_title, scaled_rect(pad + 22.0, 366.0, 220.0, 20.0, s), &primary);
            let clear_brush = solid(ctx, &theme::rgb(theme::accent::muted(), 1.0))?;
            draw_text(
                ctx,
                "CLEAR",
                &self.fmt_tab,
                scaled_rect(hits.clear.x, hits.clear.y, hits.clear.w, hits.clear.h, s),
                &clear_brush,
            );

            if p.shelf_paths.is_empty() {
                let empty = solid(ctx, &theme::rgb(theme::accent::muted(), 1.0))?;
                draw_text(
                    ctx,
                    "Drag files here.",
                    &self.fmt_body,
                    scaled_rect(pad, 400.0, 300.0, 18.0, s),
                    &empty,
                );
            }

            for (index, path) in p.shelf_paths.iter().take(5).enumerate() {
                let row = hits.rows[index];
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                let ext = std::path::Path::new(path)
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let file_color = match ext.as_str() {
                    "pdf" => theme::color::DANGER,
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => theme::color::SUCCESS,
                    "zip" | "7z" | "rar" => theme::color::WARNING,
                    _ => theme::color::INFO,
                };
                let file_brush = solid(ctx, &theme::rgb(file_color, 1.0))?;
                draw_text(ctx, "\u{E8A5}", &self.fmt_icon_sm, scaled_rect(row.x, row.y, 16.0, row.h, s), &file_brush);
                draw_text(ctx, &name, &self.fmt_small, scaled_rect(row.x + 22.0, row.y, row.w - 70.0, row.h, s), &primary);
                draw_text(
                    ctx,
                    "OPEN",
                    &self.fmt_tab,
                    scaled_rect(row.x + row.w - 44.0, row.y, 44.0, row.h, s),
                    &clear_brush,
                );
                if index < 4 {
                    ctx.FillRectangle(
                        &D2D_RECT_F {
                            left: row.x * s,
                            top: (row.y + row.h) * s,
                            right: (row.x + row.w) * s,
                            bottom: (row.y + row.h + 1.0) * s,
                        },
                        &divider,
                    );
                }
            }
        }
        if hub.is_some() {
            unsafe {
                self.ctx.PopAxisAlignedClip();
                self.ctx.SetTransform(&windows_numerics::Matrix3x2::identity());
            }
        }
        Ok(())
    }

    /// Hub overlay: every widget at once, fading and sliding in with `p.t`.
    fn draw_hub(&self, p: &DrawParams) -> Result<()> {
        use crate::state::hub;
        let s = p.scale;
        let t = p.t.clamp(0.0, 1.0);
        let slide = (1.0 - t) * 16.0;
        unsafe {
            let ctx = &self.ctx;
            let params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: D2D_RECT_F {
                    left: -1.0e6,
                    top: -1.0e6,
                    right: 1.0e6,
                    bottom: 1.0e6,
                },
                geometricMask: std::mem::ManuallyDrop::new(None),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: windows_numerics::Matrix3x2::identity(),
                opacity: t,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            };
            ctx.PushLayer(&params, None);
            ctx.SetTransform(&windows_numerics::Matrix3x2::translation(0.0, slide * s));

            for card in hub::cards() {
                draw_glass(
                    ctx,
                    scaled_rect(card.x, card.y, card.w, card.h, s),
                    20.0 * s,
                    p.opacity,
                    Glass::Card,
                    s,
                )?;
            }

            let primary = solid(ctx, &theme::rgb(theme::accent::text1(), 1.0))?;
            let secondary = solid(ctx, &theme::rgb(theme::accent::text2(), 1.0))?;
            let muted = solid(ctx, &theme::rgb(theme::accent::muted(), 1.0))?;
            let accent = solid(ctx, &theme::rgb(theme::accent::c500(), 1.0))?;

            // Launcher.
            let l = hub::launcher();
            draw_glass(ctx, scaled_rect(l.x + 12.0, l.y + 14.0, l.w - 24.0, 40.0, s), 12.0 * s, 1.0, Glass::Tile, s)?;
            draw_text(ctx, "\u{E721}", &self.fmt_icon_sm, scaled_rect(l.x + 24.0, l.y + 14.0, 20.0, 40.0, s), &secondary);
            draw_text(ctx, "Apps", &self.fmt_card_title, scaled_rect(l.x + 52.0, l.y + 14.0, 160.0, 40.0, s), &primary);
            draw_text(ctx, "pinned", &self.fmt_small, scaled_rect(l.x + 100.0, l.y + 14.0, 120.0, 40.0, s), &muted);
            if p.apps.is_empty() {
                draw_text(ctx, "No pinned apps found.", &self.fmt_body, scaled_rect(l.x + 24.0, l.y + 80.0, 280.0, 20.0, s), &muted);
            }
            for (index, name) in p.apps.iter().take(hub::app_rows()).enumerate() {
                let row = hub::app_row(index);
                let cy = row.y + row.h * 0.5;
                let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                draw_glass(ctx, scaled_rect(row.x, row.y + 6.0, 40.0, 40.0, s), 10.0 * s, 1.0, Glass::Tile, s)?;
                draw_text(ctx, &initial, &self.fmt_card_title_center, scaled_rect(row.x, row.y + 6.0, 40.0, 40.0, s), &accent);
                draw_text(ctx, name, &self.fmt_wide_title, scaled_rect(row.x + 54.0, cy - 17.0, row.w - 60.0, 18.0, s), &primary);
                draw_text(ctx, "App", &self.fmt_wide_sub, scaled_rect(row.x + 54.0, cy + 2.0, 120.0, 16.0, s), &muted);
            }

            // Clipboard.
            let c = hub::clip();
            draw_text(ctx, "Clipboard", &self.fmt_card_title, scaled_rect(c.x + 20.0, c.y + 12.0, 200.0, 22.0, s), &primary);
            let clear = hub::clip_clear();
            draw_glass(ctx, scaled_rect(clear.x, clear.y, clear.w, clear.h, s), 8.0 * s, 1.0, Glass::Tile, s)?;
            draw_text(ctx, "Clear all", &self.fmt_tab, scaled_rect(clear.x, clear.y, clear.w, clear.h, s), &secondary);
            if p.clip_items.is_empty() {
                draw_text(ctx, "Copy something to see it here.", &self.fmt_body, scaled_rect(c.x + 20.0, c.y + 52.0, 300.0, 20.0, s), &muted);
            }
            for (index, text) in p.clip_items.iter().take(hub::CLIP_ROWS).enumerate() {
                let row = hub::clip_row(index);
                let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                draw_text(ctx, "\u{E77F}", &self.fmt_icon_sm, scaled_rect(row.x, row.y, 20.0, row.h, s), &accent);
                draw_text(ctx, &line, &self.fmt_body, scaled_rect(row.x + 28.0, row.y, row.w - 28.0, row.h, s), &primary);
            }

            // Media + system.
            let m = hub::media();
            let art_rect = scaled_rect(m.x + 20.0, m.y + 20.0, 88.0, 88.0, s);
            if let Some(art) = &self.art_bitmap {
                draw_rounded_art(ctx, art, art_rect, 14.0 * s, 1.0)?;
            } else {
                draw_glass(ctx, art_rect, 14.0 * s, 1.0, Glass::Tile, s)?;
                draw_logo(ctx, &self.fmt_logo, m.x + 64.0, m.y + 64.0, 16.0, s, 1.0)?;
            }
            let (title, artist) = if p.media.available {
                (p.media.title.as_str(), p.media.artist.as_str())
            } else {
                ("Nothing playing", "Play something to control it here")
            };
            draw_text(ctx, title, &self.fmt_wide_title, scaled_rect(m.x + 124.0, m.y + 20.0, 176.0, 20.0, s), &primary);
            draw_text(ctx, artist, &self.fmt_wide_sub, scaled_rect(m.x + 124.0, m.y + 42.0, 176.0, 16.0, s), &secondary);
            let (prev, play, next) = (hub::prev(), hub::play(), hub::next());
            draw_text(ctx, "\u{E892}", &self.fmt_icon, scaled_rect(prev.x, prev.y, prev.w, prev.h, s), &secondary);
            let disc = violet_gradient(ctx, &scaled_rect(play.x, play.y, play.w, play.h, s), 1.0)?;
            ctx.FillEllipse(&circle((play.x + play.w * 0.5) * s, (play.y + play.h * 0.5) * s, play.w * 0.5 * s), &disc);
            let white = solid(ctx, &theme::rgb(0xFFFFFF, 1.0))?;
            draw_text(
                ctx,
                if p.media.playing { "\u{E769}" } else { "\u{E768}" },
                &self.fmt_icon,
                scaled_rect(play.x, play.y, play.w, play.h, s),
                &white,
            );
            draw_text(ctx, "\u{E893}", &self.fmt_icon, scaled_rect(next.x, next.y, next.w, next.h, s), &secondary);
            let bar = hub::progress();
            let ratio = if p.media.duration > 0.0 {
                (p.media.position / p.media.duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            draw_slider(ctx, bar.x, bar.y + 7.0, bar.w, ratio, p.media.available, s, 1.0)?;
            draw_text(
                ctx,
                &format!("{} / {}", format_time(p.media.position), format_time(p.media.duration)),
                &self.fmt_small,
                scaled_rect(bar.x, bar.y + 20.0, bar.w, 16.0, s),
                &muted,
            );
            let divider = solid(ctx, &theme::rgb(theme::accent::light(), 0.08))?;
            ctx.FillRectangle(&scaled_rect(m.x + 312.0, m.y + 20.0, 1.0, m.h - 40.0, s), &divider);
            let battery = p.battery.map(|b| b as f32).unwrap_or(0.0);
            let rings = [
                ("CPU", p.cpu.clamp(0.0, 100.0), format!("{:.0}%", p.cpu)),
                ("RAM", p.ram.clamp(0.0, 100.0), format!("{:.0}%", p.ram)),
                (
                    if p.charging { "POWER" } else { "BATT" },
                    battery,
                    p.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "N/A".into()),
                ),
            ];
            for (index, (label, value, text)) in rings.iter().enumerate() {
                let cx = m.x + 340.0 + index as f32 * 46.0;
                let cy = m.y + 66.0;
                let color = if index == 2 {
                    if *value <= 15.0 { theme::color::DANGER } else { theme::accent::battery() }
                } else if *value >= theme::metric::DANGER_THRESHOLD {
                    theme::color::DANGER
                } else if *value >= theme::metric::WARNING_THRESHOLD {
                    theme::color::WARNING
                } else {
                    theme::accent::c600()
                };
                draw_ring(ctx, cx, cy, 17.0, 3.5, value / 100.0, &theme::rgb(color, 1.0), s, 1.0)?;
                draw_text(ctx, text, &self.fmt_tab, scaled_rect(cx - 17.0, cy - 8.0, 34.0, 16.0, s), &primary);
                draw_text(ctx, label, &self.fmt_tab, scaled_rect(cx - 22.0, cy + 22.0, 44.0, 14.0, s), &muted);
            }
            draw_text(ctx, &p.date, &self.fmt_small, scaled_rect(m.x + 326.0, m.y + 120.0, 124.0, 16.0, s), &secondary);
            draw_text(ctx, &p.time, &self.fmt_card_title, scaled_rect(m.x + 326.0, m.y + 138.0, 124.0, 24.0, s), &primary);

            // Focus.
            let f = hub::focus();
            draw_text(ctx, "Focus", &self.fmt_card_title, scaled_rect(f.x + 20.0, f.y + 12.0, 120.0, 22.0, s), &primary);
            draw_text(
                ctx,
                if p.focus_running { "Running" } else { "Paused" },
                &self.fmt_small,
                scaled_rect(f.x + 80.0, f.y + 12.0, 120.0, 22.0, s),
                if p.focus_running { &accent } else { &muted },
            );
            let remaining = p.focus_remaining.max(0.0) as u32;
            draw_text(
                ctx,
                &format!("{:02}:{:02}", remaining / 60, remaining % 60),
                &self.fmt_focus,
                scaled_rect(f.x + 12.0, f.y + 40.0, 196.0, 64.0, s),
                &primary,
            );
            for (rect, label) in [
                (hub::focus_toggle(), if p.focus_running { "Pause" } else { "Start" }),
                (hub::focus_reset(), "Reset"),
            ] {
                draw_glass(ctx, scaled_rect(rect.x, rect.y, rect.w, rect.h, s), 10.0 * s, 1.0, Glass::Tile, s)?;
                draw_text(ctx, label, &self.fmt_tab, scaled_rect(rect.x, rect.y, rect.w, rect.h, s), &primary);
            }
            draw_slider(ctx, f.x + 20.0, f.y + 124.0, f.w - 40.0, 1.0 - p.focus_remaining / 1500.0, false, s, 1.0)?;

            // Dock.
            let docked = p.apps.len().min(hub::DOCK_MAX);
            if let Some(dock) = hub::dock(docked) {
                draw_glass(ctx, scaled_rect(dock.x, dock.y, dock.w, dock.h, s), 22.0 * s, p.opacity, Glass::Card, s)?;
                for (index, name) in p.apps.iter().take(docked).enumerate() {
                    let item = hub::dock_item(docked, index);
                    let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                    draw_glass(ctx, scaled_rect(item.x, item.y, item.w, item.h, s), 14.0 * s, 1.0, Glass::Tile, s)?;
                    draw_text(ctx, &initial, &self.fmt_card_title_center, scaled_rect(item.x, item.y, item.w, item.h, s), &accent);
                }
            }

            ctx.SetTransform(&windows_numerics::Matrix3x2::identity());
        }

        // Quick controls + shelf reuse the Cards content, shifted into place.
        self.draw_cards(p, Some(slide))?;
        unsafe {
            self.ctx.PopLayer();
        }
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        unsafe {
            let swapchain = self.factory.CreateSwapChainForComposition(
                &self._d3d,
                &swapchain_desc(width, height),
                None,
            )?;
            let surface: IDXGISurface = swapchain.GetBuffer(0)?;
            let bitmap = self.ctx.CreateBitmapFromDxgiSurface(&surface, None)?;
            self.ctx.SetTarget(&bitmap);
            self.visual.SetContent(&swapchain)?;
            self.dcomp.Commit()?;
            self.swapchain = swapchain;
            self._bitmap = bitmap;
            self.width = width;
            self.height = height;
        }
        Ok(())
    }
}

fn swapchain_desc(width: u32, height: u32) -> DXGI_SWAP_CHAIN_DESC1 {
    DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    }
}

fn decode_art(ctx: &ID2D1DeviceContext, bytes: &[u8]) -> Result<ID2D1Bitmap1> {
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        let buffer = bytes.to_vec();
        stream.InitializeFromMemory(&buffer)?;
        let decoder =
            factory.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)?;
        let frame = decoder.GetFrame(0)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &windows::Win32::Graphics::Imaging::GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        ctx.CreateBitmapFromWicBitmap(&converter, None)
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn metric(
    ctx: &ID2D1DeviceContext,
    label: &str,
    value: f32,
    x: f32,
    y: f32,
    width: f32,
    scale: f32,
    alpha: f32,
    primary: &D2D1_COLOR_F,
    _secondary: &D2D1_COLOR_F,
    accent: &D2D1_COLOR_F,
    track: &D2D1_COLOR_F,
    fmt_label: &IDWriteTextFormat,
    fmt_value: &IDWriteTextFormat,
) {
    let label_brush = match solid(ctx, primary) {
        Ok(brush) => brush,
        Err(_) => return,
    };
    label_brush.SetOpacity(0.75 * alpha);
    draw_text(
        ctx,
        label,
        fmt_label,
        scaled_rect(x, y, width * 0.5, 16.0, scale),
        &label_brush,
    );

    let value_brush = match solid(ctx, primary) {
        Ok(brush) => brush,
        Err(_) => return,
    };
    value_brush.SetOpacity(alpha);
    draw_text(
        ctx,
        &format!("{value:.0}%"),
        fmt_value,
        scaled_rect(x + width * 0.5, y, width * 0.5, 16.0, scale),
        &value_brush,
    );

    let track_brush = match solid(ctx, track) {
        Ok(brush) => brush,
        Err(_) => return,
    };
    track_brush.SetOpacity(alpha);
    let bar_y = y + 20.0;
    let bar_h = 3.0;
    let full = D2D1_ROUNDED_RECT {
        rect: scaled_rect_raw(x, bar_y, width, bar_h, scale),
        radiusX: bar_h * 0.5 * scale,
        radiusY: bar_h * 0.5 * scale,
    };
    ctx.FillRoundedRectangle(&full, &track_brush);

    let fill_w = width * (value.clamp(0.0, 100.0) / 100.0);
    if fill_w <= 0.5 {
        return;
    }
    let fill_color = if value >= theme::metric::DANGER_THRESHOLD {
        theme::rgb(theme::color::DANGER, 1.0)
    } else if value >= theme::metric::WARNING_THRESHOLD {
        theme::rgb(theme::color::WARNING, 1.0)
    } else {
        *accent
    };
    let fill_brush = match solid(ctx, &fill_color) {
        Ok(brush) => brush,
        Err(_) => return,
    };
    fill_brush.SetOpacity(alpha);
    let filled = D2D1_ROUNDED_RECT {
        rect: scaled_rect_raw(x, bar_y, fill_w, bar_h, scale),
        radiusX: bar_h * 0.5 * scale,
        radiusY: bar_h * 0.5 * scale,
    };
    ctx.FillRoundedRectangle(&filled, &fill_brush);
}

fn scaled_rect(x: f32, y: f32, w: f32, h: f32, s: f32) -> D2D_RECT_F {
    scaled_rect_raw(x, y, w, h, s)
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{}:{:02}", total / 60, total % 60)
}

fn scaled_rect_raw(x: f32, y: f32, w: f32, h: f32, s: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x * s,
        top: y * s,
        right: (x + w) * s,
        bottom: (y + h) * s,
    }
}

unsafe fn solid(ctx: &ID2D1DeviceContext, color: &D2D1_COLOR_F) -> Result<ID2D1SolidColorBrush> {
    ctx.CreateSolidColorBrush(color, None)
}

unsafe fn draw_text(
    ctx: &ID2D1DeviceContext,
    text: &str,
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let brush: ID2D1Brush = match brush.cast() {
        Ok(brush) => brush,
        Err(_) => return,
    };
    ctx.DrawText(
        &wide,
        format,
        &rect,
        &brush,
        D2D1_DRAW_TEXT_OPTIONS_CLIP,
        DWRITE_MEASURING_MODE_NATURAL,
    );
}

fn format(
    factory: &IDWriteFactory,
    family: &str,
    size: f32,
    weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    align: windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT,
    paragraph: windows::Win32::Graphics::DirectWrite::DWRITE_PARAGRAPH_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    let wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let fmt = factory.CreateTextFormat(
            windows::core::PCWSTR::from_raw(wide.as_ptr()),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            windows::core::w!("en-us"),
        )?;
        let _ = fmt.SetTextAlignment(align);
        let _ = fmt.SetParagraphAlignment(paragraph);
        Ok(fmt)
    }
}

/// Brand gradient (Violet 500 -> Violet 700), diagonal across `rect`.
unsafe fn violet_gradient(
    ctx: &ID2D1DeviceContext,
    rect: &D2D_RECT_F,
    alpha: f32,
) -> Result<ID2D1LinearGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: theme::rgb(theme::accent::c500(), alpha),
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: theme::rgb(theme::accent::c700(), alpha),
        },
    ];
    let collection = ctx.CreateGradientStopCollection(
        &stops,
        D2D1_COLOR_SPACE_SRGB,
        D2D1_COLOR_SPACE_SRGB,
        D2D1_BUFFER_PRECISION_8BPC_UNORM,
        D2D1_EXTEND_MODE_CLAMP,
        D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
    )?;
    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
        startPoint: Vector2 { X: rect.left, Y: rect.top },
        endPoint: Vector2 { X: rect.right, Y: rect.bottom },
    };
    ctx.CreateLinearGradientBrush(&props, None, &collection)
}

unsafe fn circle(cx: f32, cy: f32, r: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: Vector2 { X: cx, Y: cy },
        radiusX: r,
        radiusY: r,
    }
}

/// NOTCH app mark: a glowing violet disc with a bold "N". Coordinates in DIPs.
unsafe fn draw_logo(
    ctx: &ID2D1DeviceContext,
    fmt: &IDWriteTextFormat,
    cx: f32,
    cy: f32,
    r: f32,
    s: f32,
    alpha: f32,
) -> Result<()> {
    let glow = solid(ctx, &theme::rgb(theme::color::VIOLET_600, 0.22 * alpha))?;
    ctx.FillEllipse(&circle(cx * s, cy * s, (r + 3.0) * s), &glow);
    let rect = scaled_rect(cx - r, cy - r, r * 2.0, r * 2.0, s);
    let fill = linear(
        ctx,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        &[
            (0.0, theme::rgb(theme::color::VIOLET_500, alpha)),
            (1.0, theme::rgb(theme::color::VIOLET_700, alpha)),
        ],
    )?;
    ctx.FillEllipse(&circle(cx * s, cy * s, r * s), &fill);
    let rim = solid(ctx, &theme::rgb(theme::color::VIOLET_300, 0.45 * alpha))?;
    ctx.DrawEllipse(&circle(cx * s, cy * s, r * s), &rim, 1.0, None);
    let text = solid(ctx, &theme::rgb(0xFFFFFF, alpha))?;
    draw_text(ctx, "N", fmt, rect, &text);
    Ok(())
}

/// Album artwork clipped to a rounded square, with a hairline rim.
unsafe fn draw_rounded_art(
    ctx: &ID2D1DeviceContext,
    art: &ID2D1Bitmap1,
    rect: D2D_RECT_F,
    radius: f32,
    alpha: f32,
) -> Result<()> {
    let bitmap: ID2D1Bitmap = art.cast()?;
    let size = bitmap.GetSize();
    let brush = ctx.CreateBitmapBrush(&bitmap, None, None)?;
    let sx = (rect.right - rect.left) / size.width.max(1.0);
    let sy = (rect.bottom - rect.top) / size.height.max(1.0);
    brush.SetTransform(&windows_numerics::Matrix3x2 {
        M11: sx,
        M12: 0.0,
        M21: 0.0,
        M22: sy,
        M31: rect.left,
        M32: rect.top,
    });
    brush.SetOpacity(alpha);
    let rr = D2D1_ROUNDED_RECT {
        rect,
        radiusX: radius,
        radiusY: radius,
    };
    ctx.FillRoundedRectangle(&rr, &brush);
    let rim = solid(ctx, &theme::rgb(theme::accent::light(), 0.10 * alpha))?;
    ctx.DrawRoundedRectangle(&rr, &rim, 1.0, None);
    Ok(())
}

/// Circular gauge: graphite track plus a violet arc from 12 o'clock, clockwise.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_ring(
    ctx: &ID2D1DeviceContext,
    cx: f32,
    cy: f32,
    r: f32,
    thickness: f32,
    ratio: f32,
    color: &D2D1_COLOR_F,
    s: f32,
    alpha: f32,
) -> Result<()> {
    let track = solid(ctx, &theme::rgb(theme::accent::track(), alpha))?;
    ctx.DrawEllipse(&circle(cx * s, cy * s, r * s), &track, thickness * s, None);
    let ratio = ratio.clamp(0.0, 1.0);
    if ratio <= 0.005 {
        return Ok(());
    }
    let arc = solid(ctx, color)?;
    arc.SetOpacity(alpha);
    let steps = (72.0 * ratio).ceil().max(2.0) as usize;
    let sweep = std::f32::consts::TAU * ratio;
    let point = |a: f32| Vector2 {
        X: (cx + r * a.sin()) * s,
        Y: (cy - r * a.cos()) * s,
    };
    for i in 0..steps {
        let a0 = sweep * i as f32 / steps as f32;
        let a1 = sweep * (i + 1) as f32 / steps as f32;
        ctx.DrawLine(point(a0), point(a1), &arc, thickness * s, None);
    }
    // Round caps at both ends.
    for a in [0.0, sweep] {
        let p = point(a);
        ctx.FillEllipse(&circle(p.X, p.Y, thickness * 0.5 * s), &arc);
    }
    Ok(())
}

/// Slider: graphite track, violet gradient fill, white thumb.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_slider(
    ctx: &ID2D1DeviceContext,
    x: f32,
    y: f32,
    w: f32,
    ratio: f32,
    thumb: bool,
    s: f32,
    alpha: f32,
) -> Result<()> {
    let h = 4.0;
    let track = solid(ctx, &theme::rgb(theme::accent::track(), alpha))?;
    ctx.FillRoundedRectangle(
        &D2D1_ROUNDED_RECT {
            rect: scaled_rect(x, y, w, h, s),
            radiusX: h * 0.5 * s,
            radiusY: h * 0.5 * s,
        },
        &track,
    );
    let ratio = ratio.clamp(0.0, 1.0);
    if ratio > 0.001 {
        let rect = scaled_rect(x, y, w * ratio, h, s);
        let fill = violet_gradient(ctx, &scaled_rect(x, y, w, h, s), alpha)?;
        ctx.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect,
                radiusX: h * 0.5 * s,
                radiusY: h * 0.5 * s,
            },
            &fill,
        );
    }
    if thumb {
        let cx = (x + w * ratio) * s;
        let cy = (y + h * 0.5) * s;
        let glow = solid(ctx, &theme::rgb(theme::accent::c600(), 0.30 * alpha))?;
        ctx.FillEllipse(&circle(cx, cy, 8.0 * s), &glow);
        let knob = solid(ctx, &theme::rgb(0xFFFFFF, alpha))?;
        ctx.FillEllipse(&circle(cx, cy, 5.5 * s), &knob);
    }
    Ok(())
}

/// Round icon badge (Segoe MDL2 glyph), violet when active.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_icon_badge(
    ctx: &ID2D1DeviceContext,
    fmt: &IDWriteTextFormat,
    glyph: &str,
    cx: f32,
    cy: f32,
    r: f32,
    active: bool,
    s: f32,
    alpha: f32,
) -> Result<()> {
    let rect = scaled_rect(cx - r, cy - r, r * 2.0, r * 2.0, s);
    if active {
        let fill = violet_gradient(ctx, &rect, alpha)?;
        ctx.FillEllipse(&circle(cx * s, cy * s, r * s), &fill);
    } else {
        let fill = solid(ctx, &theme::rgb(theme::accent::track(), alpha))?;
        ctx.FillEllipse(&circle(cx * s, cy * s, r * s), &fill);
    }
    let color = if active {
        theme::rgb(0xFFFFFF, alpha)
    } else {
        theme::rgb(theme::accent::text2(), alpha)
    };
    let brush = solid(ctx, &color)?;
    draw_text(ctx, glyph, fmt, rect, &brush);
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Glass {
    /// The notch / island itself. No outer shadow: the window hugs the shape.
    Surface,
    /// Floating desktop card with a soft drop shadow.
    Card,
    /// Inner tile or chip sitting on another glass layer.
    Tile,
}

/// Glassmorphism surface: translucent tinted body, diagonal sheen, a light
/// rim that is brighter on top, and (for cards) a soft drop shadow.
/// `rect` and `radius` are already in pixels.
unsafe fn draw_glass(
    ctx: &ID2D1DeviceContext,
    rect: D2D_RECT_F,
    radius: f32,
    opacity: f32,
    kind: Glass,
    s: f32,
) -> Result<()> {
    let g = &theme::glass::GLASS;
    let rr = |r: D2D_RECT_F, rad: f32| D2D1_ROUNDED_RECT {
        rect: r,
        radiusX: rad,
        radiusY: rad,
    };

    if kind == Glass::Card {
        for i in 1..=4 {
            let grow = i as f32 * 2.0 * s;
            let shadow = solid(ctx, &theme::rgb(0x000000, g.shadow / i as f32 * opacity))?;
            ctx.FillRoundedRectangle(
                &rr(
                    D2D_RECT_F {
                        left: rect.left - grow * 0.5,
                        top: rect.top + grow * 0.5,
                        right: rect.right + grow * 0.5,
                        bottom: rect.bottom + grow,
                    },
                    radius + grow,
                ),
                &shadow,
            );
        }
    }

    let (top, bottom) = if kind == Glass::Tile {
        (
            theme::rgb(theme::accent::light(), g.tile_fill * 1.4 * opacity),
            theme::rgb(theme::accent::light(), g.tile_fill * 0.6 * opacity),
        )
    } else {
        (
            theme::rgb(theme::accent::tint(), g.body_top * opacity),
            theme::rgb(theme::accent::base(), g.body_bottom * opacity),
        )
    };
    let body = linear(ctx, rect.left, rect.top, rect.left, rect.bottom, &[(0.0, top), (1.0, bottom)])?;
    ctx.FillRoundedRectangle(&rr(rect, radius), &body);

    // Diagonal sheen, strongest at the top-left corner.
    let sheen_a = if kind == Glass::Tile { g.sheen * 0.5 } else { g.sheen };
    let sheen = linear(
        ctx,
        rect.left,
        rect.top,
        rect.left + (rect.right - rect.left) * 0.55,
        rect.bottom,
        &[
            (0.0, theme::rgb(theme::accent::light(), sheen_a * opacity)),
            (0.45, theme::rgb(theme::accent::light(), sheen_a * 0.25 * opacity)),
            (1.0, theme::rgb(theme::accent::light(), 0.0)),
        ],
    )?;
    ctx.FillRoundedRectangle(&rr(rect, radius), &sheen);

    // Rim: bright on top, violet through the middle, fading at the bottom.
    let rim_a = if kind == Glass::Tile { g.rim * 0.6 } else { g.rim };
    let rim = linear(
        ctx,
        rect.left,
        rect.top,
        rect.left,
        rect.bottom,
        &[
            (0.0, theme::rgb(theme::accent::light(), rim_a * opacity)),
            (0.5, theme::rgb(theme::accent::c500(), rim_a * 0.55 * opacity)),
            (1.0, theme::rgb(theme::accent::light(), rim_a * 0.2 * opacity)),
        ],
    )?;
    let inset = 0.5 * s;
    ctx.DrawRoundedRectangle(
        &rr(
            D2D_RECT_F {
                left: rect.left + inset,
                top: rect.top + inset,
                right: rect.right - inset,
                bottom: rect.bottom - inset,
            },
            (radius - inset).max(0.0),
        ),
        &rim,
        1.0 * s,
        None,
    );
    Ok(())
}

unsafe fn linear(
    ctx: &ID2D1DeviceContext,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    stops: &[(f32, D2D1_COLOR_F)],
) -> Result<ID2D1LinearGradientBrush> {
    let stops: Vec<D2D1_GRADIENT_STOP> = stops
        .iter()
        .map(|(position, color)| D2D1_GRADIENT_STOP {
            position: *position,
            color: *color,
        })
        .collect();
    let collection = ctx.CreateGradientStopCollection(
        &stops,
        D2D1_COLOR_SPACE_SRGB,
        D2D1_COLOR_SPACE_SRGB,
        D2D1_BUFFER_PRECISION_8BPC_UNORM,
        D2D1_EXTEND_MODE_CLAMP,
        D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
    )?;
    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
        startPoint: Vector2 { X: x0, Y: y0 },
        endPoint: Vector2 { X: x1, Y: y1 },
    };
    ctx.CreateLinearGradientBrush(&props, None, &collection)
}

/// Pure black "hardware" surface for the iPhone-style modes. `rect` and
/// `radius` are in pixels. NotchX adds concave shoulders where the notch
/// meets the top edge; IslandPro gets a faint rim so it reads on dark
/// wallpapers the way the real island does under glass.
unsafe fn draw_hardware(
    ctx: &ID2D1DeviceContext,
    rect: D2D_RECT_F,
    radius: f32,
    mode: crate::state::SurfaceMode,
    s: f32,
) -> Result<()> {
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D_SIZE_F,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_SWEEP_DIRECTION_CLOCKWISE,
        D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
    };

    let black = solid(ctx, &theme::rgb(0x000000, 1.0))?;
    let rr = D2D1_ROUNDED_RECT {
        rect,
        radiusX: radius,
        radiusY: radius,
    };
    ctx.FillRoundedRectangle(&rr, &black);

    if mode == crate::state::SurfaceMode::NotchX {
        let f = 9.0 * s;
        let factory = ctx.GetFactory()?;
        let path = factory.CreatePathGeometry()?;
        let sink = path.Open()?;
        let p = |x: f32, y: f32| Vector2 { X: x, Y: y };
        let arc = |x: f32, y: f32, sweep| D2D1_ARC_SEGMENT {
            point: p(x, y),
            size: D2D_SIZE_F { width: f, height: f },
            rotationAngle: 0.0,
            sweepDirection: sweep,
            arcSize: D2D1_ARC_SIZE_SMALL,
        };
        let (l, r) = (rect.left, rect.right);
        // Left shoulder.
        sink.BeginFigure(p(l - f, 0.0), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(p(l + 1.0, 0.0));
        sink.AddLine(p(l + 1.0, f));
        sink.AddLine(p(l, f));
        sink.AddArc(&arc(l - f, 0.0, D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        // Right shoulder.
        sink.BeginFigure(p(r + f, 0.0), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(p(r - 1.0, 0.0));
        sink.AddLine(p(r - 1.0, f));
        sink.AddLine(p(r, f));
        sink.AddArc(&arc(r + f, 0.0, D2D1_SWEEP_DIRECTION_CLOCKWISE));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        ctx.FillGeometry(&path, &black, None);
    } else {
        let rim = solid(ctx, &theme::rgb(0xFFFFFF, 0.07))?;
        let inset = 0.5 * s;
        ctx.DrawRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: rect.left + inset,
                    top: rect.top + inset,
                    right: rect.right - inset,
                    bottom: rect.bottom - inset,
                },
                radiusX: (radius - inset).max(0.0),
                radiusY: (radius - inset).max(0.0),
            },
            &rim,
            1.0 * s,
            None,
        );
    }
    Ok(())
}

/// Front camera lens: dark glass with a deep blue iris and a tiny glint.
unsafe fn draw_lens(ctx: &ID2D1DeviceContext, cx: f32, cy: f32, s: f32, alpha: f32) -> Result<()> {
    let ring = solid(ctx, &theme::rgb(0x15161C, alpha))?;
    ctx.FillEllipse(&circle(cx * s, cy * s, 6.0 * s), &ring);
    let iris = solid(ctx, &theme::rgb(0x1B2350, alpha))?;
    ctx.FillEllipse(&circle(cx * s, cy * s, 3.6 * s), &iris);
    let pupil = solid(ctx, &theme::rgb(0x05060A, alpha))?;
    ctx.FillEllipse(&circle(cx * s, cy * s, 2.0 * s), &pupil);
    let glint = solid(ctx, &theme::rgb(0x8FA3FF, 0.55 * alpha))?;
    ctx.FillEllipse(&circle((cx - 1.4) * s, (cy - 1.4) * s, 0.9 * s), &glint);
    Ok(())
}
