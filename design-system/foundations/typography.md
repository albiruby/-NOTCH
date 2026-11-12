# Typography Foundation

## Keluarga font

| Peran | Font | Fallback |
| --- | --- | --- |
| UI | Segoe UI Variable | Segoe UI, Arial, sans-serif |
| Teknis | Cascadia Code | Cascadia Mono, Consolas, monospace |

Segoe UI Variable dipakai untuk navigasi, heading, label, deskripsi, dan kontrol.
Cascadia Code dipakai selektif: angka metrik, timestamp, ID, nilai diagnostik, dan kode.

Monospace tidak dipakai untuk paragraf.

## Type scale

| Token | Size | Weight | Penggunaan |
| --- | --- | --- | --- |
| type.display | 32 | 600 | Angka utama, jam besar |
| type.h1 | 24 | 600 | Heading halaman |
| type.h2 | 20 | 600 | Heading seksi |
| type.h3 | 16 | 600 | Subseksi |
| type.title | 14 | 600 | Judul komponen |
| type.body | 14 | 400 | Teks utama |
| type.body-sm | 13 | 400 | Teks sekunder, jam di pill |
| type.label | 12 | 500 | Label |
| type.caption | 11 | 400 | Metadata |
| type.micro | 10 | 500 | Penanda ringkas |

Line height:

- Display dan heading: 1.2 sampai 1.35.
- Body: 1.4 sampai 1.55.
- Label ringkas: 1.2 sampai 1.3.

## Aturan

- Sentence case untuk hampir semua label.
- Uppercase hanya untuk metadata teknis, dan tidak lebih dari satu baris.
- Angka yang berubah memakai tabular numerals.
- Jangan menurunkan teks penting di bawah 11 px.
- Dukung Windows text scaling.
- Jangan memakai lebih dari dua weight dalam satu komponen.

## Pemetaan ke rendering

DirectWrite membuat `IDWriteTextFormat` per kombinasi keluarga, ukuran, dan weight. Format dibuat sekali saat startup, bukan per frame.

| Format | Font | Size | Weight |
| --- | --- | --- | --- |
| clock.compact | Segoe UI Variable | 13 | 600 |
| clock.display | Segoe UI Variable | 32 | 600 |
| label | Segoe UI Variable | 12 | 500 |
| caption | Segoe UI Variable | 11 | 400 |
| metric.value | Cascadia Code | 12 | 600 |
| wordmark | Segoe UI Variable | 10 | 600 |

Angka jam di pill memakai `clock.compact`. Angka CPU dan memori memakai `metric.value`, sehingga lebarnya stabil saat nilainya berubah.
