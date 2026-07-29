use crate::config::resolve_path;
use image::{imageops::FilterType, ImageBuffer, Rgb};
use ndarray::Array4;
use ort::session::Session;
use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;

/// Konstanta untuk MoveNet
const MODEL_INPUT_SIZE: u32 = 192;
const KEYPOINT_LEFT_EAR: usize = 4;
const KEYPOINT_LEFT_SHOULDER: usize = 6;
const VERTICAL_REFERENCE_OFFSET: f32 = 100.0;

/// Memuat model ONNX dari folder model/ dengan validasi path traversal.
///
/// Melakukan `canonicalize()` pada setiap file `.onnx` yang ditemukan dan
/// memastikan path canonical-nya berada di dalam folder `model/`.
/// Ini mencegah symlink berbahaya yang mengarah ke luar folder model.
pub fn load_model() -> Option<Session> {
    let model_dir = resolve_path("model");
    let model_dir_path = PathBuf::from(&model_dir);

    // Canonicalize folder model untuk validasi path traversal
    let model_dir_canonical = match model_dir_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "❌ Folder 'model/' tidak ditemukan atau tidak dapat diakses: {}",
                e
            );
            return None;
        }
    };

    let mut model_path = None;
    if let Ok(entries) = fs::read_dir(&model_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("onnx") {
                // Validasi path traversal: pastikan file berada di dalam folder model/
                if let Ok(canonical) = path.canonicalize() {
                    if canonical.starts_with(&model_dir_canonical) {
                        model_path = Some(canonical.to_string_lossy().to_string());
                        break;
                    } else {
                        eprintln!(
                            "⚠️ Melewati file di luar folder model (kemungkinan symlink): {:?}",
                            path
                        );
                    }
                }
            }
        }
    }

    if let Some(path) = model_path {
        println!("Mencoba memuat model ONNX dari {}...", path);
        let mut builder = match Session::builder() {
            Ok(b) => match b.with_intra_threads(1) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("❌ Gagal memuat model ONNX: {}", e);
                    return None;
                }
            },
            Err(e) => {
                eprintln!("❌ Gagal memuat model ONNX: {}", e);
                return None;
            }
        };

        match builder.commit_from_file(&path) {
            Ok(s) => {
                let inputs = s.inputs();
                println!("✅ Model ONNX berhasil dimuat!");
                if let Some(input0) = inputs.first() {
                    println!("INFO MODEL - Name: {}", input0.name());
                }
                Some(s)
            }
            Err(e) => {
                eprintln!("❌ Model ONNX gagal dimuat: {}", e);
                None
            }
        }
    } else {
        eprintln!("❌ Tidak ditemukan satupun file berformat .onnx di dalam folder 'model/'.");
        None
    }
}

/// Proses frame kamera dan hitung sudut leher menggunakan model MoveNet.
///
/// Menerima raw buffer RGB dari kamera (zero-copy, bukan .to_vec()),
/// melakukan resize ke 192x192, konversi ke tensor float32 secara bulk,
/// lalu inferensi model dan hitung sudut antara telinga-bahu-vertikal.
pub fn calculate_neck_angle(
    session: &mut Session,
    buffer: &[u8],
    width: u32,
    height: u32,
) -> Result<f32, String> {
    // Zero-copy: pinjam buffer langsung tanpa alokasi
    let img = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, buffer)
        .ok_or_else(|| "Gagal membuat ImageBuffer dari frame".to_string())?;

    // Resize ke ukuran input model
    let resized = image::imageops::resize(
        &img,
        MODEL_INPUT_SIZE,
        MODEL_INPUT_SIZE,
        FilterType::Nearest,
    );

    // Bulk tensor creation: konversi langsung dari raw buffer (1 pass, cache-friendly)
    // Memory layout ImageBuffer [H, W, 3] row-major = ndarray (1, H, W, 3) C-order
    let raw = resized.as_raw();
    let float_data: Vec<f32> = raw.iter().map(|&b| b as f32).collect();
    let tensor = Array4::from_shape_vec(
        (1, MODEL_INPUT_SIZE as usize, MODEL_INPUT_SIZE as usize, 3),
        float_data,
    )
    .map_err(|e| format!("Gagal membuat tensor: {}", e))?;

    let tensor_value = ort::value::Tensor::from_array(tensor)
        .map_err(|e| format!("Gagal konversi tensor ke ort::Value: {}", e))?;

    let inputs = ort::inputs![tensor_value];
    let outputs = session
        .run(inputs)
        .map_err(|e| format!("Gagal inferensi model: {:?}", e))?;

    let output_value = &outputs[0];
    let (shape, extracted) = output_value
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Gagal membaca output tensor: {:?}", e))?;

    let view = ndarray::ArrayView4::from_shape(
        (shape[0] as usize, shape[1] as usize, shape[2] as usize, shape[3] as usize),
        extracted,
    )
    .map_err(|e| format!("Gagal membuat ArrayView: {}", e))?;
    let size = MODEL_INPUT_SIZE as f32;

    // Ekstrak keypoint Telinga dan Bahu
    // Format output MoveNet: [1, 1, 17, 3] — [0] = Y, [1] = X
    let ear_y = view[[0, 0, KEYPOINT_LEFT_EAR, 0]] * size;
    let ear_x = view[[0, 0, KEYPOINT_LEFT_EAR, 1]] * size;

    let shoulder_y = view[[0, 0, KEYPOINT_LEFT_SHOULDER, 0]] * size;
    let shoulder_x = view[[0, 0, KEYPOINT_LEFT_SHOULDER, 1]] * size;

    // Titik referensi vertikal di atas bahu
    let p3_x = shoulder_x;
    let p3_y = shoulder_y - VERTICAL_REFERENCE_OFFSET;

    let angle_ear_shoulder = (ear_y - shoulder_y).atan2(ear_x - shoulder_x);
    let angle_vert_shoulder = (p3_y - shoulder_y).atan2(p3_x - shoulder_x);
    let mut theta = (angle_ear_shoulder - angle_vert_shoulder).abs() * 180.0 / PI;
    if theta > 180.0 {
        theta = 360.0 - theta;
    }

    Ok(theta)
}