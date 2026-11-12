# Accessibility Foundation

Baseline: WCAG 2.2 AA, disesuaikan untuk interaksi desktop native.

## Kontras

- Teks normal minimal 4.5:1.
- Teks besar minimal 3:1.
- Teks di atas glass harus diuji pada latar terburuk, bukan latar rata-rata.
- Kalau kontras gagal di salah satu kondisi, opacity surface dinaikkan.

Kombinasi aman:

| Teks | Latar | Rasio |
| --- | --- | --- |
| graphite-100 `#E8EAF0` | graphite-800 `#171A20` | di atas 12:1 |
| graphite-300 `#A0A6B2` | graphite-800 `#171A20` | di atas 6:1 |
| violet-300 `#D4CEFF` | graphite-900 `#101216` | di atas 9:1 |

## Status tanpa warna

Status tidak boleh hanya dibedakan lewat warna. Setiap status membawa minimal dua dari:

- Teks label.
- Ikon atau bentuk.
- Posisi atau urutan.

Contoh: metrik di atas ambang menampilkan warna warning dan nilai persentase, bukan hanya warna.

## Keyboard

- Semua kontrol bisa dicapai dengan Tab.
- Enter atau Space mengaktifkan.
- Escape menutup overlay.
- Focus indicator terlihat, memakai `color.border.focus` dengan ketebalan 2 px.
- Urutan fokus mengikuti urutan visual.

## Motion dan transparansi

- Reduced Motion: animasi non-esensial dimatikan, transisi langsung.
- Reduced Transparency: blur dimatikan, opacity dinaikkan.
- High Contrast: token semantic dipetakan ke palet kontras tinggi Windows.

## Skala dan DPI

Diuji pada 100%, 125%, 150%, dan 200%.

- Dimensi layout dalam DIP.
- Konversi ke pixel memakai DPI monitor aktif.
- Teks mengikuti Windows text scaling tanpa terpotong.

## Hit target

Minimal 40 DIP, ideal 44 DIP. Kalau kontrol visual lebih kecil, area interaksi tetap diperbesar.

## Label

- Kontrol ikon saja wajib punya tooltip dan accessible name.
- Nama harus menjelaskan aksi, bukan bentuk ikon.
- Hindari label "OK" tanpa konteks. Gunakan "Simpan", "Hapus", "Tutup".

## Checklist sebelum rilis

- [ ] Semua teks lolos kontras pada latar terburuk.
- [ ] Tidak ada status yang hanya bergantung pada warna.
- [ ] Focus terlihat di semua kontrol.
- [ ] Reduced Motion dan Reduced Transparency berfungsi.
- [ ] Diuji pada empat tingkat DPI.
- [ ] Hit target minimal 40 DIP.
- [ ] Tidak ada kontrol tanpa label.
