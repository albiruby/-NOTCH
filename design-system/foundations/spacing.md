# Spacing, Sizing, Radius, Border

## Spacing

Base 4 px. Nilai di luar skala harus punya alasan yang ditulis.

| Token | Nilai | Contoh |
| --- | --- | --- |
| space.1 | 4 | Jarak ikon kecil |
| space.2 | 8 | Label ke ikon |
| space.3 | 12 | Padding kontrol ringkas |
| space.4 | 16 | Padding kartu |
| space.5 | 20 | Panel kecil |
| space.6 | 24 | Padding panel, jarak seksi |
| space.8 | 32 | Pemisah besar |
| space.10 | 40 | Jarak layout |
| space.12 | 48 | Pemisah workspace |
| space.16 | 64 | Ruang kosong besar |

## Sizing

| Token | Nilai |
| --- | --- |
| icon.xs | 12 |
| icon.sm | 16 |
| icon.md | 18 |
| icon.lg | 20 |
| icon.xl | 24 |
| control.compact | 28 |
| control.standard | 36 |
| control.large | 44 |
| hitTarget | 44 |
| sidebar | 240 |
| panelMin | 280 |

Target sentuh 40 sampai 44 DIP meskipun kontrol terlihat lebih kecil.

## Radius

| Token | Nilai | Penggunaan |
| --- | --- | --- |
| radius.xs | 4 | Indikator kecil |
| radius.sm | 6 | Tombol ringkas |
| radius.md | 10 | Input, kartu kecil |
| radius.lg | 16 | Kartu standar |
| radius.xl | 22 | Panel mengambang |
| radius.2xl | 28 | Workspace |
| radius.pill | 999 | Pill tengah |

Hierarki bentuk harus jelas. Jangan membuat semua elemen pill.

## Border

| Token | Nilai |
| --- | --- |
| border.width.default | 1 |
| border.width.emphasis | 2 |
| border.color.subtle | `#2B303A` |
| border.color.strong | `#414856` |
| border.color.focus | `#8B7CFF` |

Border dipakai untuk memisahkan surface atau menandai state, bukan dekorasi.

## Penerapan di surface

| Surface | Padding | Radius |
| --- | --- | --- |
| Central pill | 16 horizontal, 0 vertikal | pill |
| Expanded surface | 24 | xl |
| Widget | 16 | lg |
| Workspace | 32 | 2xl |
| Settings | 24 | lg |

Dimensi disimpan dalam DIP dan dikalikan DPI saat render.
