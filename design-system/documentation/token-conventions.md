# Token Conventions

## Penamaan

```
{category}.{role}.{variant}.{state}
```

Contoh:

```
color.bg.surface
color.text.primary
color.accent.primary
button.primary.background.hover
surface.centralPill.blur
motion.panel.expand.duration
```

Nama menjelaskan peran, bukan nilai. `color.accent.primary` benar. `purple-500-button` salah.

## Lapisan

| Lapisan | Boleh dirujuk oleh | Tidak boleh dirujuk oleh |
| --- | --- | --- |
| Primitive | Semantic | Komponen, surface |
| Semantic | Component, surface | Renderer langsung, kecuali lewat resolver |
| Component | Surface, renderer | Primitive |
| Surface | Renderer | Primitive |

## Format file

- JSON dengan komentar tidak didukung. Dokumentasi tambahan masuk ke `.md`.
- Referensi antar token memakai `{dot.path}`.
- Nilai dimensi dalam DIP, tanpa satuan di JSON.
- Warna dalam hex `#RRGGBB`, alpha terpisah.
- Durasi dalam milidetik.

## Validasi

Skrip validasi (rencana) memeriksa:

1. Semua referensi `{...}` menunjuk token yang ada.
2. Tidak ada referensi melingkar.
3. Setiap token di `components.json` dan `surfaces.json` bisa di-resolve.
4. Tidak ada primitive yang dipakai langsung oleh komponen.

## Perubahan

- Perubahan nilai: patch.
- Token baru: minor.
- Perubahan struktur atau nama: major, disertai migrasi.

Setiap perubahan dicatat di `documentation/changelog.md` saat file itu dibuat.
