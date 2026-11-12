# Motion Foundation

## Prinsip

Animasi menjelaskan perubahan state, bukan mengisi ruang.

- Tidak ada loop animasi saat idle.
- Hover tidak memicu animasi besar.
- Expand dan collapse harus terasa cepat.
- Drag mengikuti pointer tanpa lag.
- Entrance dan exit harus konsisten.
- Reduced Motion mengurangi atau mematikan animasi non-esensial.

## Durasi

| Token | Nilai | Penggunaan |
| --- | --- | --- |
| motion.instant | 80 ms | Umpan balik kecil |
| motion.fast | 120 ms | Hover, press |
| motion.normal | 180 ms | Expand ringkas |
| motion.smooth | 220 ms | Transisi surface |
| motion.slow | 300 ms | Transisi besar |

## Easing

| Token | Karakter | Penggunaan |
| --- | --- | --- |
| ease.standard | Seimbang | Transisi umum |
| ease.enter | Deselerasi lembut | Surface muncul |
| ease.exit | Akselerasi | Surface menghilang |
| ease.spring | Spring terkendali | Snap dan morph |

Spring harus critically damped atau sedikit di bawahnya. Overshoot berlebihan dilarang.

## Implementasi

NOTCH memakai spring kritis dengan `omega = 30`, dihitung per frame:

```
x = value - target
accel = -omega^2 * x - 2 * omega * velocity
velocity += accel * dt
value += velocity * dt
```

Frame loop memakai waitable timer resolusi tinggi yang dibaca lewat `MsgWaitForMultipleObjectsEx` dengan periode 16 ms. `WM_TIMER` tidak dipakai untuk animasi karena prioritasnya rendah dan resolusinya tidak stabil.

Hasil ukur saat ini: rata-rata 16.30 ms per frame, maksimum 16.56 ms, durasi satu transisi sekitar 358 ms.

## Mapping ke token durasi

Spring tidak memakai durasi tetap. Nilai `motion.*` dipakai untuk animasi berbasis waktu seperti fade dan hover. Untuk morph surface, spring menjadi acuan rasa, dan `motion.smooth` menjadi batas atas.

Kalau nanti dibutuhkan durasi eksplisit, gunakan pemetaan:

| Transisi | Token |
| --- | --- |
| Press | motion.instant |
| Hover | motion.fast |
| Expand pill ke panel | motion.smooth |
| Collapse | motion.normal |
| Notification masuk | motion.smooth |
| Notification keluar | motion.fast |
