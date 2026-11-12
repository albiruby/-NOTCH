# Implementation Guidelines

## Lapisan

```
primitives.json  ->  nilai mentah
semantic.json    ->  peran
components.json  ->  gaya komponen
surfaces.json    ->  konfigurasi surface
        |
        v
Theme resolver (Rust)
        |
        v
Resolved style  ->  renderer
```

Komponen tidak membaca primitive. Kalau sebuah komponen butuh warna yang belum ada di semantic, tambahkan token, jangan hardcode.

## Struktur Rust

```
src/
├─ theme.rs          token dan resolver
├─ renderer.rs       Direct2D, DirectWrite
├─ state.rs          geometri dan spring
├─ main.rs           lifecycle, event, tray
├─ window.rs         penempatan, DPI, fullscreen
├─ sysmon.rs         sampling CPU dan memori
├─ settings.rs       konfigurasi pengguna
└─ tray.rs           tray icon
```

`theme.rs` adalah satu-satunya tempat nilai warna dan ukuran boleh didefinisikan.
Renderer mengambil nilai dari sana.

## Aturan renderer

1. Dimensi disimpan dalam DIP.
2. Kalikan dengan skala DPI saat menggambar.
3. Buat `IDWriteTextFormat` sekali saat startup.
4. Jangan resolve token per frame.
5. Perbarui hanya area yang berubah kalau memungkinkan.
6. Sediakan fallback solid kalau blur tidak tersedia.

## Konversi warna

Token disimpan sebagai hex `#RRGGBB`. Renderer memakai `D2D1_COLOR_F` dengan nilai 0.0 sampai 1.0 dan alpha terpisah.

```rust
pub const fn rgb(hex: u32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: alpha,
    }
}
```

## Tema dan preset

- Preset mengubah semantic token, bukan komponen.
- Custom token divalidasi sebelum diterapkan.
- Nilai yang tidak valid kembali ke default dan dicatat di log.
- Perubahan tema hanya me-resolve ulang style, bukan membuat ulang window.

## Performa

- Tidak ada animasi saat idle.
- Tidak ada timer yang berjalan tanpa kebutuhan.
- Modul yang tidak terlihat tidak melakukan sampling.
- Cache style yang sudah di-resolve.
- Ukur ulang setiap kali menambah efek.

## Checklist implementasi komponen baru

1. Tentukan surface dan density.
2. Pakai token yang sudah ada. Kalau tidak ada, tambahkan ke semantic.
3. Definisikan state: default, hover, focus, active, disabled.
4. Definisikan data state: loading, empty, unavailable, error.
5. Uji pada empat tingkat DPI.
6. Uji dengan Reduced Motion.
