# Halo Ces
Perbaiki carata duduk.

## Konfigurasi

Sesuaikan preferensi dengan format parameternya:

```json
{
  "baseline": {
    "average_neck_angle": 14.0362425
  },
  "debounce_threshold_frames": 3,
  "tolerance_angle": 15.0
}
```

### Penjelasan Parameter:

- **`baseline.average_neck_angle`**: Sudut leher rata-rata yang dianggap normal/ideal (baseline) untuk pengguna saat duduk tegak. Anda mungkin perlu menyesuaikan nilai ini sesuai dengan sudut tangkapan kamera dan postur alami Anda.
- **`tolerance_angle`**: Toleransi perubahan sudut (dalam derajat) sebelum sistem menganggap postur tubuh sudah buruk atau berubah secara signifikan dari baseline.
- **`debounce_threshold_frames`**: Jumlah frame (tangkapan kamera) yang dibutuhkan secara berturut-turut untuk memastikan bahwa perubahan postur itu valid. Ini berguna untuk mencegah sistem memberikan peringatan palsu hanya karena Anda bergerak cepat atau menoleh sesaat.

## Model ONNX

Letakkan file .onnx ke folder model/.