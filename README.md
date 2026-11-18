# NOTCH

Notch native untuk Windows. Satu file `.exe`, tanpa Electron, tanpa WebView2,
tanpa runtime tambahan.

Dibangun dengan Rust + Win32 + DirectComposition/Direct2D. Visual tree dipegang
kompositor Windows, jadi aplikasi hanya menggambar ketika ada yang berubah.

## Status

v1.13 — AI Developer Cockpit di Hub. v1.12: indikator privasi kamera/mikrofon dan Live Activities lewat CLI. Kaca hampir bening; theme (Violet, Aurora Mint, Sunset Coral) mengubah seluruh tampilan: rona kaca, teks, track, tile, dan tepi. Mode iPhone X notch dan Dynamic Island (Pro). Hub memenuhi layar dengan tata letak lega ala referensi. Dynamic island bisa di-drag bebas dan menempel ke tepi/tengah layar. Layout Hub jadi default (klik pill, semua widget muncul). Glassmorphism penuh: permukaan tembus pandang, kilau diagonal, tepi bercahaya, bayangan lembut. Visual mengikuti referensi brand. Yang sudah ada:

**Empat layout**
- `Hub` (default): klik pill, semua widget muncul sekaligus dan memenuhi layar: kolom kiri (Apps launcher, Clipboard), tengah atas (Media + CPU/RAM/baterai), kolom kanan (Focus timer, Quick Controls, File Shelf), dan dock aplikasi di bawah tengah. Bagian tengah layar dibiarkan kosong. Pill berubah jadi top bar. Klik area kosong atau top bar untuk menutup.
- `Panel`: panel vertikal dengan 7 halaman.
- `Wide bar`: satu bar panjang berisi artwork, judul/artis, transport, progress, CPU/RAM/baterai, dan jam. Klik bar untuk membuka panel halaman penuh; setelah panel ditutup, bar lebar kembali otomatis.
- `Cards`: dua kartu mengambang di kanan bawah. Quick Controls berisi volume, brightness, Wi-Fi/BT, Focus, Battery, Record, dan Theme. File Shelf berisi path file yang di-drag, klik untuk membuka, Clear untuk mengosongkan.
- Pilih lewat tray → **Layout**.

**Bentuk dan perilaku**
- Empat gaya (tray): Classic notch (glass), **iPhone X notch** (hitam pekat, menempel di atas, dengan bahu melengkung ke dalam di kedua sudut), Dynamic island (glass), dan **Dynamic Island (Pro)** (pil hitam pekat seperti iPhone 14 Pro ke atas, dengan lensa kamera di ujung kanan)
- Dynamic island bisa di-drag ke mana saja. Dilepas dekat tepi kiri/kanan, tengah, atas, atau bawah layar, pill menempel ke sana dengan animasi spring. Posisi tersimpan; tray → **Reset island position** mengembalikan ke atas tengah
- Hover untuk expand, mouse keluar untuk collapse, animasi spring kritis
- Klik di luar pill menembus ke aplikasi di bawahnya
- Tidak mencuri keyboard focus saat idle
- Auto-hide saat aplikasi fullscreen, topmost, multi-monitor, DPI per-monitor
- Tray icon: show/hide, ganti gaya, opacity glass, hide on fullscreen, start with Windows

**Media**
- Judul, artis, status putar, progress, elapsed/duration
- Album artwork dari thumbnail sesi Windows (decode WIC → bitmap D2D)
- Prev / play-pause / next, klik progress bar untuk seek

**Lirik**
- Lirik sinkron dari LRCLIB, tiga baris di sekitar posisi lagu
- Diambil sekali per lagu lewat worker thread, cache di memori

**Kontrol dan overlay**
- Volume master dengan slider (WASAPI), termasuk mute
- Brightness dengan slider (Dxva2), tampil N/A kalau panel tidak mendukung
- Wi-Fi dan Bluetooth lewat WinRT radios, tampil N/A kalau tidak tersedia
- Overlay singkat saat tombol volume hardware ditekan, lewat callback WASAPI
- Overlay Caps Lock lewat low-level keyboard hook (tidak menelan tombol)

**Privasi**
- Titik hijau (kamera) dan oranye (mikrofon) di pill saat ada aplikasi yang memakainya, dibaca dari `CapabilityAccessManager` Windows tiap 2 detik
- Saat aplikasi baru mulai memakai kamera/mic, pill menampilkan `CAMERA · Zoom` / `MIC · Discord` sebentar

**Live Activities (CLI)**
- Program atau script apa pun bisa menampilkan progres di island:
  ```powershell
  notch push "Build" --progress 40 --detail "cargo build --release"
  notch push "Build" --done
  notch push "Deploy" --fail --detail "exit code 1"
  notch clear            # atau: notch clear "Build"
  ```
- Pill menampilkan aktivitas terbaru (ikon status, judul, persen, garis progres). Hub menampilkan sampai 3 aktivitas di kartu **Live Activities** di bawah kartu media
- Aktivitas dengan judul yang sama diperbarui, bukan ditambah. Yang selesai/gagal hilang setelah 6 detik

**Deteksi sesi AI aktif** (di kartu cockpit)
- Baris "Active now" menampilkan aplikasi AI yang sedang terbuka: Claude, ChatGPT, Gemini, DeepSeek, Copilot, Cursor, Perplexity, lewat nama proses dan judul tab browser
- Hanya membaca yang sudah terlihat sistem (daftar proses, judul jendela). Tidak menyentuh akun, cookie, token, atau isi percakapan aplikasi lain
- Berjalan tanpa API key sama sekali

**AI Developer Cockpit** (Hub, kanan atas)
- Biaya bulan berjalan dan token per penyedia: **Claude** (Usage and Cost Analytics API), **OpenAI** (Costs and Usage API organisasi), dan saldo **DeepSeek**
- Bar anggaran bulanan, sisa hari di bulan ini, dan grafik biaya harian
- Butuh **admin key** (bukan key biasa): Claude `sk-ant-admin...` dari Console → Admin keys; OpenAI admin key dari Organization settings
  ```powershell
  notch key set claude sk-ant-admin01-...
  notch key set openai sk-admin-...
  notch key set deepseek sk-...
  notch ai budget 200        # USD per bulan, 0 = tanpa bar anggaran
  notch key clear openai
  notch ai refresh
  ```
- Key disimpan di **Windows Credential Manager** (`NOTCH/ai/<provider>`), tidak pernah di `settings.json`. Data diperbarui tiap 15 menit dan saat Hub dibuka jika lebih dari 5 menit
- Catatan: data biaya Claude biasanya tertunda sekitar 4 jam (maks. 24 jam)

**Cuaca & jaringan** (kartu media Hub)
- Cuaca saat ini dari `wttr.in` (tanpa API key, lokasi deteksi via IP), diperbarui tiap 30 menit
- Throughput jaringan download/upload di bawah jam

**Produktivitas**
- Focus timer Pomodoro 25 menit, klik untuk start/pause, reset
- Tasks dengan input keyboard langsung di panel, tersimpan di `tasks.json`
- Water tracker 8 gelas per hari, reset otomatis saat tanggal berganti, pengingat 45 menit
- File shelf: drag file ke notch, klik untuk membuka; path disimpan, file tidak disalin
- Clipboard history 8 item terakhir, klik untuk menyalin ulang; hanya di memori, tidak ditulis ke disk
- Ikon aplikasi asli di launcher dan dock (diekstrak dari shortcut Windows), fallback ke huruf awal kalau ikon tak terbaca
- Apps launcher: shortcut pinned taskbar (fallback Start Menu), klik untuk membuka aplikasi asli

**Halaman panel**
`OVERVIEW`, `CONTROLS`, `FOCUS`, `TASKS`, `SHELF`, `CLIP`, `APPS`.

## Hasil ukur

Diukur pada Windows 11 build 26300, 1920x1080 @ 125%, release build.

| Metrik | Hasil |
| --- | --- |
| Ukuran exe | 0.36 MB |
| RAM idle (working set) | 56 MB |
| RAM idle (private) | 40 MB |
| CPU idle | 0.0% |
| CPU expanded | ~0.16% (sampling media, radio, lirik) |
| Thread | 12 |
| Frame interval | 16.30 ms rata-rata, 16.56 ms maksimum |
| Durasi expand | ~360 ms |

RAM masih di atas target 15–30 MB. Penyebab utamanya stack GPU
(D3D11 + DirectComposition + Direct2D + DirectWrite), bukan logika aplikasi.

Sampling media, radio, dan lirik hanya berjalan saat panel terbuka. Saat
compact, aplikasi hanya bangun untuk jam (timer 1 detik hanya aktif saat
expanded), event volume, Caps Lock, clipboard, dan drop file.

Animasi memakai waitable timer resolusi tinggi yang dibaca lewat
`MsgWaitForMultipleObjectsEx`, bukan `WM_TIMER`, sehingga frame tidak
tertunda saat antrean pesan ramai. Gerakannya memakai spring kritis
(`state::Spring`), bukan easing dengan durasi tetap.

## Build

```powershell
cargo build --release
```

Hasil: `target\release\notch.exe`.

Butuh Rust stable MSVC dan Visual Studio Build Tools (C++ workload).

## Install

```powershell
powershell -ExecutionPolicy Bypass -File installer\install.ps1
```

Script menyalin exe ke `%LOCALAPPDATA%\Notch`, membuat shortcut Start Menu,
dan (kecuali `-NoAutostart`) mendaftarkan autostart di
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

Uninstall:

```powershell
powershell -ExecutionPolicy Bypass -File installer\uninstall.ps1
```

Paket zip untuk distribusi:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

## Menjalankan

```powershell
cargo run --release
```

Atau langsung `target\release\notch.exe`.

Keluar lewat tray icon, klik kanan lalu **Quit NOTCH**.

Untuk pengujian, `notch.exe --expanded` langsung membuka panel tanpa hover.

## Interaksi

| Aksi | Hasil |
| --- | --- |
| Drag pill (dynamic island) | Pindahkan; dilepas dekat tepi/tengah akan menempel |
| Klik pill (Hub) | Buka semua widget sekaligus |
| Klik area kosong / top bar (Hub) | Tutup Hub |
| Hover pill (Panel/Wide) | Buka panel atau wide bar sesuai layout |
| Klik wide bar | Buka panel halaman penuh |
| Drag slider di kartu | Ubah volume/brightness |
| Klik chip Focus | Start/pause timer fokus |
| Klik Battery / Record / Theme | Buka setting baterai / Game Bar / warna Windows |
| Drag file ke kartu Shelf | Tambah path ke shelf |
| Klik tab | Ganti halaman |
| Klik progress bar | Seek |
| Klik artwork / judul | Tidak ada aksi (placeholder untuk player switch) |
| Klik task | Tandai selesai |
| Klik `+ ADD TASK` | Mulai mengetik; Enter simpan, Esc batal |
| Klik titik air | Set jumlah gelas |
| Drag file ke panel | Tambah ke shelf dan buka halaman SHELF |
| Klik item shelf | Buka file dengan aplikasi default |
| Klik item clipboard | Salin ulang ke clipboard |
| Klik item di APPS | Buka aplikasi yang di-pin |
| Tombol volume hardware | Overlay volume muncul 1.5 detik |
| Caps Lock | Overlay status muncul 1.5 detik |

## Settings

Dibuat otomatis di `%APPDATA%\Notch\`.

| File | Isi |
| --- | --- |
| `settings.json` | mode, opacity, ukuran pill/panel, hide_on_fullscreen, show_clock |
| `ai.json` | anggaran bulanan AI Cockpit (`budget_usd`) |
| `tasks.json` | daftar task, jumlah air harian, target air |
| `shelf.json` | path file yang di-drop (maksimal 8) |

| Field | Arti |
| --- | --- |
| `mode` | `notch`, `notch_x`, `island` (default), atau `island_pro` |
| `theme` | `violet` (default), `mint`, atau `coral` |
| `layout` | `hub` (default), `panel`, `wide`, atau `cards` |
| `opacity` | Opasitas background (0.2–1.0) |
| `compact_width` / `compact_height` | Ukuran pill |
| `expanded_width` / `expanded_height` | Ukuran panel |
| `hide_on_fullscreen` | Sembunyikan saat fullscreen |
| `show_clock` | Tampilkan jam di pill |
| `island_anchor` | Posisi pusat island sebagai pecahan layar `[x, y]`, `null` = atas tengah |

## Privasi

- Tidak ada akun, telemetri, atau analytics.
- Clipboard history hanya hidup di memori proses dan hilang saat keluar.
- Lirik dikirim ke `lrclib.net` sebagai judul + artis lagu yang sedang diputar.
- Semua data lain tetap di komputer.

## Struktur

```
src/
├─ main.rs       message loop, state, tray, kebijakan visibilitas
├─ window.rs     penempatan, DPI, deteksi fullscreen
├─ renderer.rs   DirectComposition + Direct2D + DirectWrite
├─ state.rs      geometri, tab, hit rect, animasi
├─ theme.rs      design tokens (warna, ukuran, gerak)
├─ settings.rs   konfigurasi JSON
├─ media.rs      GSMTC worker, artwork, transport, seek
├─ lyrics.rs     LRCLIB + parser LRC + WinHTTP
├─ audio.rs      WASAPI volume + callback hardware
├─ brightness.rs Dxva2 brightness
├─ netstatus.rs  WinRT radio status
├─ tasks.rs      tasks + water, JSON
├─ shelf.rs      file shelf, JSON
├─ ai.rs        AI cockpit: worker, provider fetch, CLI key/budget
├─ weather.rs   cuaca dari wttr.in
├─ sessions.rs  deteksi sesi AI aktif (proses + judul jendela)
├─ ai_report.rs parsing respons + tanggal (murni, ada unit test)
├─ http.rs      HTTPS GET via WinHTTP
├─ secrets.rs   Windows Credential Manager
├─ activity.rs  Live Activities + CLI push/clear
├─ privacy.rs   indikator kamera/mikrofon
├─ apps.rs       pinned taskbar / Start Menu launcher
├─ clipboard.rs  clipboard history (memori)
├─ sysmon.rs     sampling CPU dan memori
└─ tray.rs       tray icon dan menu
```

## Design system

Token dan aturan visual ada di `design-system/`. Sumber nilai ada di
`design-system/tokens/*.json`, dan `src/theme.rs` memetakan nilai itu ke renderer.
Mulai dari `design-system/README.md`.

## Batasan yang diketahui

- Preset `NOTCH Desktop Overview` belum lengkap. Yang sudah ada: top bar (Wide),
  dua kartu Quick Controls + File Shelf (Cards), dan launcher APPS. Command palette
  dengan pencarian, notification center, AI cockpit dengan data nyata, meeting,
  ambient orb, dan dock belum ada.
- Backdrop blur belum ada. Glass saat ini memakai tint transparan tanpa blur
  karena surface DirectComposition belum punya jalur capture di belakangnya.
- Membaca notifikasi aplikasi lain butuh identitas paket Microsoft Store; tidak
  dilakukan. NOTCH tidak membaca notifikasi aplikasi lain.
- Shelf menerima drag-in dan klik-untuk-buka; drag-out ke Explorer belum ada.
- Album artwork bergantung pada aplikasi pemutar yang mengekspos thumbnail ke
  GSMTC. Sebagian aplikasi tidak melakukannya.
- Wi-Fi/Bluetooth bergantung pada radio yang diekspos ke aplikasi desktop;
  di sebagian mesin nilainya N/A.
