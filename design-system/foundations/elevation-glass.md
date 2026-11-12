# Elevation and Glass

## Elevation

| Level | Perlakuan | Penggunaan |
| --- | --- | --- |
| elevation.0 | Tanpa shadow | Base, canvas |
| elevation.1 | Border halus | Surface standar |
| elevation.2 | Shadow sedang | Panel mengambang |
| elevation.3 | Shadow kuat | Overlay kritis |

Shadow bukan default untuk semua komponen. Level 0 dan 1 harus mendominasi.

## Glass

Keputusan v1.1: **strong glass**. Surface lebih transparan dan membawa tint violet,
bukan sekadar graphite netral.

| Token | Nilai |
| --- | --- |
| glass.opacity.subtle | 0.68 |
| glass.opacity.standard | 0.80 |
| glass.opacity.solid | 0.90 |
| glass.blur.sm | 8 DIP |
| glass.blur.md | 16 DIP |
| glass.blur.lg | 24 DIP |
| glass.border.opacity | 0.30 |
| glass.highlight.opacity | 0.10 |

Selain opacity, surface memakai gradient vertikal:

| Posisi | Warna | Alpha |
| --- | --- | --- |
| 0% | violet-950 `#171329` | 0.90 |
| 62% | graphite-900 `#101216` | 0.86 |
| 100% | graphite-950 `#090A0D` | 0.84 |

Tint violet di bagian atas memberi identitas, graphite di bawah menjaga keterbacaan.

## Pemakaian

| Surface | Material |
| --- | --- |
| Central pill | strong glass |
| Expanded surface | strong glass |
| Widget | standard glass |
| Notification | standard glass |
| Context menu | solid |
| Settings | solid |
| Tabel data padat | solid |

## Aturan

- Glass tidak boleh jadi syarat keterbacaan. Teks harus tetap lolos kontras pada latar terburuk.
- Blur dibatasi pada surface yang terlihat, bukan seluruh layar.
- Satu surface maksimal satu lapisan blur.
- Di mode hemat daya atau reduced transparency, blur dimatikan dan opacity dinaikkan.

## Fallback

Rendering harus mendukung empat mode:

1. Strong glass, blur aktif.
2. Reduced transparency, blur dimatikan, opacity 0.90.
3. Solid, tanpa blur, opacity 1.0.
4. Battery saver, efek dimatikan dan animasi dipercepat.

Renderer memilih mode berdasarkan settings dan kapabilitas GPU. Kontrak komponen tidak berubah.

## Catatan implementasi

Blur di Windows Composition memerlukan efek tambahan (`IDCompositionDevice3` dan effect graph)
atau material DWM. Sampai layer efek tersedia, NOTCH merender gradient tint dengan alpha
0.84 sampai 0.90, yang tetap terbaca tanpa blur.
