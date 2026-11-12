# NOTCH Design System v1.0

Status: foundation approved.
Platform: Windows 10 22H2+ / Windows 11.
Rendering: Rust native, DirectComposition + Direct2D + DirectWrite.

## Keputusan yang dikunci

| Area | Keputusan |
| --- | --- |
| Visual identity | Graphite + Electric Violet |
| Glass intensity | Strong glass, tinted violet, dengan fallback solid |
| Typography | Segoe UI Variable + Cascadia Code |
| Customization | Preset + custom tokens |
| Motion | 80 sampai 300 ms, berbasis tujuan |
| Accessibility | WCAG 2.2 AA sebagai baseline |

## Arah visual

**Native Precision x Balanced Glass x Technical Minimalism.**

Enam prinsip:

1. Clarity over decoration. Keterbacaan lebih dulu.
2. Function over ornament. Setiap elemen punya fungsi.
3. Context over complexity. Tampilkan yang relevan saja.
4. Consistency over novelty. Komponen dipakai bersama.
5. Performance by design. Tidak ada efek yang mahal tanpa alasan.
6. Customization with boundaries. Kustomisasi tidak boleh merusak usability.

## Komponen v1.0

| Komponen | Aturan |
| --- | --- |
| Wide bar | Tinggi 140 DIP, lebar 900 DIP; media 0..560, metrik CPU/RAM/baterai 576..764, jam rata kanan 778..878 |
| Tab bar | 7 tab, lebar 60 DIP, label MICRO semibold, underline violet 2 px hanya di tab aktif |
| Task row | Checkbox 12 DIP, teks LABEL, task selesai diredupkan ke 35% |
| Water row | 8 dot 7 DIP, terisi memakai accent, target harian di kanan |
| Shelf row | Nama file LEADING, aksi `OPEN` MICRO 30% di kanan, divider 1 px |
| Overlay pill | Menggantikan jam di compact pill selama 1.5 detik, teks di atas bar 2.5 DIP |
| Media block | Artwork 40 DIP, teks trailing, lirik tiga baris saat tersedia |

Semua komponen memakai token semantik yang sudah ada; tidak ada warna atau
ukuran baru di luar `theme.rs`.

## Struktur

```
design-system/
├─ README.md
├─ foundations/
│  ├─ color.md
│  ├─ typography.md
│  ├─ spacing.md
│  ├─ elevation-glass.md
│  ├─ motion.md
│  └─ accessibility.md
├─ tokens/
│  ├─ primitives.json
│  ├─ semantic.json
│  ├─ components.json
│  └─ surfaces.json
└─ documentation/
   ├─ implementation-guidelines.md
   └─ token-conventions.md
```

## Token sebagai source of truth

Empat lapisan, tidak boleh dilompati:

1. **Primitive**: nilai mentah, misalnya `graphite-900`, `violet-600`, `space.4`.
2. **Semantic**: peran, misalnya `color.bg.surface`, `color.text.primary`.
3. **Component**: gaya per komponen, misalnya `panel.background`, `metric.value`.
4. **Surface**: konfigurasi per surface, misalnya `centralPill`, `expandedSurface`.

Komponen tidak boleh memakai primitive secara langsung. Semua akses lewat semantic atau component token.

## Runtime

Rust mengonsumsi token yang sudah di-resolve. Alurnya:

```
primitives.json -> semantic.json -> components.json -> surfaces.json
        -> Theme resolver (Rust) -> resolved style -> renderer
```

Aturan renderer:

- Simpan dimensi dalam DIP, kalikan dengan DPI monitor saat menggambar.
- Cache style yang sudah di-resolve. Jangan resolve token per frame.
- Blur dan shadow hanya pada surface yang benar-benar butuh.
- Selalu sediakan fallback solid.
- Tidak ada animation loop saat idle.

## Versioning

Semantic versioning:

- **Major**: perubahan struktur token atau kontrak komponen.
- **Minor**: token, komponen, preset, atau opsi kustomisasi baru.
- **Patch**: koreksi nilai tanpa perubahan struktur.
