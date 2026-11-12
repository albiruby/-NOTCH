# Color Foundation

## Identitas

Graphite sebagai fondasi, Electric Violet sebagai aksen interaktif.
Violet tidak boleh mendominasi permukaan. Kalau satu layar terlihat ungu, berarti dosisnya salah.

## Primitive

### Graphite

| Token | Hex | Peran |
| --- | --- | --- |
| graphite-950 | `#090A0D` | Canvas paling dalam |
| graphite-900 | `#101216` | Background utama |
| graphite-850 | `#14171D` | Background sekunder |
| graphite-800 | `#171A20` | Surface |
| graphite-750 | `#1C2028` | Surface interaktif |
| graphite-700 | `#20242D` | Surface mengambang |
| graphite-600 | `#2B303A` | Border halus |
| graphite-500 | `#414856` | Border kuat, teks disabled |
| graphite-400 | `#707989` | Teks muted |
| graphite-300 | `#A0A6B2` | Teks sekunder |
| graphite-200 | `#C8CCD5` | Teks mendekati primer |
| graphite-100 | `#E8EAF0` | Teks primer |

### Electric Violet

| Token | Hex | Peran |
| --- | --- | --- |
| violet-950 | `#171329` | Background aksen dalam |
| violet-900 | `#292440` | Aksen subtle |
| violet-800 | `#51458F` | Border aksen |
| violet-700 | `#7464EB` | Active |
| violet-600 | `#8B7CFF` | Aksen primer |
| violet-500 | `#A194FF` | Hover |
| violet-400 | `#B9AEFF` | Highlight |
| violet-300 | `#D4CEFF` | Aksen kontras tinggi |

### Semantic

| Token | Hex |
| --- | --- |
| success | `#4ADE80` |
| warning | `#FBBF24` |
| danger | `#FB7185` |
| info | `#60A5FA` |

Warna status harus tetap bisa dibedakan dari aksen primer.

## Semantic mapping

| Token | Nilai |
| --- | --- |
| color.bg.canvas | graphite-950 |
| color.bg.base | graphite-900 |
| color.bg.surface | graphite-800 |
| color.bg.interactive | graphite-750 |
| color.bg.elevated | graphite-700 |
| color.text.primary | graphite-100 |
| color.text.secondary | graphite-300 |
| color.text.muted | graphite-400 |
| color.text.disabled | graphite-500 |
| color.border.subtle | graphite-600 |
| color.border.strong | graphite-500 |
| color.border.focus | violet-600 |
| color.accent.primary | violet-600 |
| color.accent.hover | violet-500 |
| color.accent.active | violet-700 |
| color.accent.subtle | violet-900 |
| color.accent.border | violet-800 |

## Aturan pakai

- Aksen primer hanya untuk aksi, seleksi, fokus, dan identitas modul aktif.
- Aksen subtle untuk latar seleksi dan highlight ringan.
- Graphite untuk semua struktur dan latar.
- Warna status hanya untuk status nyata, bukan dekorasi.
- Status tidak boleh disampaikan lewat warna saja. Selalu ada teks, ikon, atau bentuk.
- Dalam satu surface kecil, maksimal satu warna aksen jenuh.

## Data status

Nilai metrik memakai ambang, bukan aksen:

| Kondisi | Warna |
| --- | --- |
| Normal | color.accent.primary untuk bar, teks tetap primer |
| Warning, di atas 85% | color.state.warning |
| Danger, di atas 95% | color.state.danger |

Ambang ini ada di `components.json` pada `metric.thresholds` dan boleh diubah lewat settings.
