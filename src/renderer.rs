//! DirectComposition + Direct2D + DirectWrite surface.

use windows::core::{Interface, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D1_GRADIENT_STOP, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1Brush, ID2D1Device, ID2D1DeviceContext, ID2D1LinearGradientBrush,
    ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_INTERPOLATION_MODE_LINEAR,
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
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_MEDIUM,
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
    fmt_glyph: IDWriteTextFormat,
    fmt_status: IDWriteTextFormat,
    fmt_tab: IDWriteTextFormat,
    fmt_focus: IDWriteTextFormat,
    fmt_body: IDWriteTextFormat,
    fmt_small: IDWriteTextFormat,
    fmt_wide_value: IDWriteTextFormat,
    fmt_wide_time: IDWriteTextFormat,
    fmt_wide_title: IDWriteTextFormat,
    fmt_wide_sub: IDWriteTextFormat,
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
                fmt_glyph: format(
                    &dwrite,
                    "Segoe UI Symbol",
                    15.0,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
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
                fmt_wide_value: format(
                    &dwrite,
                    theme::typography::FONT_MONO,
                    20.0,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
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

            let border = theme::rgb(theme::color::VIOLET_800, 0.30 * p.opacity);
            let primary = theme::rgb(theme::color::TEXT_PRIMARY, 1.0);
            let secondary = theme::rgb(theme::color::TEXT_SECONDARY, 1.0);
            let accent = theme::rgb(theme::color::ACCENT_PRIMARY, 1.0);
            let track = theme::rgb(theme::color::BG_ELEVATED, 1.0);

            let rect = D2D_RECT_F {
                left: shape.x * s,
                top: shape.y * s,
                right: shape.right() * s,
                bottom: shape.bottom() * s,
            };
            let radius = shape.r * s;
            let rr = D2D1_ROUNDED_RECT {
                rect,
                radiusX: radius,
                radiusY: radius,
            };

            let stops = [
                D2D1_GRADIENT_STOP {
                    position: 0.0,
                    color: theme::rgb(
                        theme::color::VIOLET_950,
                        theme::glass::STRONG_TOP * p.opacity,
                    ),
                },
                D2D1_GRADIENT_STOP {
                    position: 0.62,
                    color: theme::rgb(
                        theme::color::GRAPHITE_900,
                        theme::glass::STRONG_MID * p.opacity,
                    ),
                },
                D2D1_GRADIENT_STOP {
                    position: 1.0,
                    color: theme::rgb(
                        theme::color::GRAPHITE_950,
                        theme::glass::STRONG_BOTTOM * p.opacity,
                    ),
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
                startPoint: Vector2 {
                    X: rect.left,
                    Y: rect.top,
                },
                endPoint: Vector2 {
                    X: rect.left,
                    Y: rect.bottom,
                },
            };
            let gradient: ID2D1LinearGradientBrush =
                ctx.CreateLinearGradientBrush(&props, None, &collection)?;
            ctx.FillRoundedRectangle(&rr, &gradient);
            let border_brush = solid(ctx, &border)?;
            ctx.DrawRoundedRectangle(&rr, &border_brush, 1.0, None);

            // A single hairline on the top edge, so the panel reads as a surface
            // catching light rather than a flat black box.
            let highlight = solid(ctx, &theme::rgb(0xFFFFFF, theme::glass::HIGHLIGHT_OPACITY))?;
            ctx.FillRectangle(
                &D2D_RECT_F {
                    left: (shape.x + shape.r) * s,
                    top: (shape.y + 1.0) * s,
                    right: (shape.right() - shape.r) * s,
                    bottom: (shape.y + 1.8) * s,
                },
                &highlight,
            );

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
                    let dot = solid(ctx, &accent)?;
                    dot.SetOpacity(compact_alpha);
                    let center_y = (visible_top + shape.bottom()) * 0.5 * s;
                    let dot_r = 2.6 * s;
                    ctx.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: (shape.x + 15.0) * s - dot_r,
                                top: center_y - dot_r,
                                right: (shape.x + 15.0) * s + dot_r,
                                bottom: center_y + dot_r,
                            },
                            radiusX: dot_r,
                            radiusY: dot_r,
                        },
                        &dot,
                    );

                    let text_brush = solid(ctx, &primary)?;
                    text_brush.SetOpacity(compact_alpha);
                    draw_text(
                        ctx,
                        &p.time,
                        &self.fmt_clock,
                        D2D_RECT_F {
                            left: (shape.x + 26.0) * s,
                            top: (visible_top + 4.0) * s,
                            right: (shape.right() - 14.0) * s,
                            bottom: (shape.bottom() - 2.0) * s,
                        },
                        &text_brush,
                    );
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

                let track_brush = solid(ctx, &track)?;
                track_brush.SetOpacity(panel_alpha);
                ctx.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: scaled_rect_raw(bar_left, y + 92.0, bar_w, 3.0, s),
                        radiusX: 1.5 * s,
                        radiusY: 1.5 * s,
                    },
                    &track_brush,
                );

                if p.media.available {
                    if ratio > 0.001 {
                        let fill_brush = solid(ctx, &accent)?;
                        fill_brush.SetOpacity(panel_alpha);
                        ctx.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: scaled_rect_raw(bar_left, y + 92.0, bar_w * ratio, 3.0, s),
                                radiusX: 1.5 * s,
                                radiusY: 1.5 * s,
                            },
                            &fill_brush,
                        );
                    }

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
                        "\u{23EE}",
                        &self.fmt_glyph,
                        scaled_rect(
                            transport.prev.x,
                            transport.prev.y,
                            transport.prev.w,
                            transport.prev.h,
                            s,
                        ),
                        &glyph_brush,
                    );

                    let play_brush = solid(ctx, &primary)?;
                    play_brush.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        if p.media.playing { "\u{23F8}" } else { "\u{25B6}" },
                        &self.fmt_glyph,
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
                        "\u{23ED}",
                        &self.fmt_glyph,
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
                            let outline = solid(ctx, &theme::rgb(theme::color::VIOLET_800, 1.0))?;
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
                    ctx.DrawBitmap(
                        art,
                        Some(&scaled_rect(x + 22.0, y + 24.0, 48.0, 48.0, s)),
                        panel_alpha,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                } else {
                    let tile = solid(ctx, &theme::rgb(theme::color::GRAPHITE_750, 0.9))?;
                    tile.SetOpacity(panel_alpha);
                    ctx.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: scaled_rect(x + 22.0, y + 24.0, 48.0, 48.0, s),
                            radiusX: 10.0 * s,
                            radiusY: 10.0 * s,
                        },
                        &tile,
                    );
                    let mark = solid(ctx, &accent)?;
                    mark.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        "N",
                        &self.fmt_glyph,
                        scaled_rect(x + 22.0, y + 24.0, 48.0, 48.0, s),
                        &mark,
                    );
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
                let track_brush = solid(ctx, &track)?;
                track_brush.SetOpacity(0.9 * panel_alpha);
                ctx.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: scaled_rect_raw(media_left, y + 74.0, bar_w, 3.0, s),
                        radiusX: 1.5 * s,
                        radiusY: 1.5 * s,
                    },
                    &track_brush,
                );
                if ratio > 0.001 {
                    let fill_brush = solid(ctx, &accent)?;
                    fill_brush.SetOpacity(panel_alpha);
                    ctx.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: scaled_rect_raw(media_left, y + 74.0, bar_w * ratio, 3.0, s),
                            radiusX: 1.5 * s,
                            radiusY: 1.5 * s,
                        },
                        &fill_brush,
                    );
                }
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
                    "\u{23EE}",
                    &self.fmt_glyph,
                    scaled_rect(wide.prev.x, wide.prev.y, wide.prev.w, wide.prev.h, s),
                    &glyph_brush,
                );
                let play_brush = solid(ctx, &primary)?;
                play_brush.SetOpacity(panel_alpha);
                draw_text(
                    ctx,
                    if p.media.playing { "\u{23F8}" } else { "\u{25B6}" },
                    &self.fmt_glyph,
                    scaled_rect(wide.play.x, wide.play.y, wide.play.w, wide.play.h, s),
                    &play_brush,
                );
                draw_text(
                    ctx,
                    "\u{23ED}",
                    &self.fmt_glyph,
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
                    let col_x = x + 576.0 + index as f32 * 64.0;
                    let m_label = solid(ctx, &secondary)?;
                    m_label.SetOpacity(0.5 * panel_alpha);
                    draw_text(
                        ctx,
                        label,
                        &self.fmt_status,
                        scaled_rect(col_x, y + 30.0, 60.0, 12.0, s),
                        &m_label,
                    );
                    let m_value = solid(ctx, &primary)?;
                    m_value.SetOpacity(panel_alpha);
                    draw_text(
                        ctx,
                        text,
                        &self.fmt_wide_value,
                        scaled_rect(col_x, y + 48.0, 60.0, 24.0, s),
                        &m_value,
                    );
                    let m_track = solid(ctx, &track)?;
                    m_track.SetOpacity(0.85 * panel_alpha);
                    ctx.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: scaled_rect_raw(col_x, y + 86.0, 58.0, 3.0, s),
                            radiusX: 1.5 * s,
                            radiusY: 1.5 * s,
                        },
                        &m_track,
                    );
                    if *value > 0.5 {
                        let m_fill = solid(ctx, &accent)?;
                        m_fill.SetOpacity(panel_alpha);
                        ctx.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: scaled_rect_raw(
                                    col_x,
                                    y + 86.0,
                                    58.0 * (value / 100.0),
                                    3.0,
                                    s,
                                ),
                                radiusX: 1.5 * s,
                                radiusY: 1.5 * s,
                            },
                            &m_fill,
                        );
                    }
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

            let ctx = &self.ctx;
            ctx.EndDraw(None, None)?;
            let hr = self.swapchain.Present(0, DXGI_PRESENT(0));
            hr.ok()?;
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
