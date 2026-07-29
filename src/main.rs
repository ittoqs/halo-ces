mod config;
mod database;
mod inference;
mod state_engine;

use config::Config;
use state_engine::{PostureState, StateEngine};

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tray_item::{IconSource, TrayItem};

fn run_posture_loop(config: Config, running: Arc<AtomicBool>) {
    let mut state_engine = StateEngine::new(config);

    if state_engine.state == PostureState::Calibrating {
        println!("Memulai kalibrasi otomatis. Harap duduk dengan posisi ideal Anda...");
    } else {
        println!("Menggunakan data kalibrasi sebelumnya dari config.json");
    }

    let index = CameraIndex::Index(0);
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

    println!("Mencoba mengakses webcam...");
    let mut camera = match Camera::new(index, requested) {
        Ok(cam) => cam,
        Err(e) => {
            eprintln!("Gagal menginisialisasi webcam: {}", e);
            return;
        }
    };
    if let Err(e) = camera.open_stream() {
        eprintln!("Gagal membuka stream webcam: {}", e);
        return;
    }
    println!("Webcam berhasil diinisialisasi.");

    let mut session = inference::load_model();

    println!("Memulai loop monitoring (Berjalan di background)...");

    let mut loop_count = 0;
    loop {
        // Cek apakah aplikasi diminta berhenti
        if !running.load(Ordering::Relaxed) {
            println!("Menerima sinyal berhenti. Menutup kamera dan loop monitoring...");
            break;
        }

        match camera.frame() {
            Ok(frame) => {
                if let Some(ref mut sess) = session {
                    let width = frame.resolution().width();
                    let height = frame.resolution().height();
                    let buffer = frame.buffer();

                    match inference::calculate_neck_angle(sess, buffer, width, height) {
                        Ok(theta) => {
                            println!("--------------------------------------");
                            println!("Frame {}: Sudut Leher {:.2}°", loop_count + 1, theta);
                            state_engine.process_angle(theta);
                        }
                        Err(e) => {
                            eprintln!("Gagal memproses frame: {}", e);
                        }
                    }
                } else {
                    println!("Tolong perbaiki model.onnx Anda. (Gagal dimuat sebelumnya)");
                }
            }
            Err(e) => {
                eprintln!("Gagal mengambil frame: {}", e);
            }
        }

        loop_count += 1;
        thread::sleep(Duration::from_secs(2));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Local Posture Detector ===");

    if let Err(e) = database::init_db() {
        eprintln!("Gagal menginisialisasi database SQLite: {}", e);
    } else {
        println!("Database riwayat (SQLite) siap.");
    }

    // Inisialisasi ORT di main thread sebelum spawn
    let _ = ort::init().with_name("posture_detector").commit();

    let config = Config::load_or_default();

    // Flag untuk graceful shutdown antar thread
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Spawn utas background
    thread::spawn(move || {
        run_posture_loop(config, running_clone);
    });

    // Inisialisasi System Tray di utas utama (Main Thread)
    #[cfg(target_os = "linux")]
    let icon = IconSource::Resource("camera-web"); // Ikon bawaan Linux
    #[cfg(target_os = "windows")]
    let icon = IconSource::Resource(""); // Default ke ikon .exe di Windows
    #[cfg(target_os = "macos")]
    let icon = IconSource::Resource(""); // Default ke ikon app bundle di Mac

    let mut tray = match TrayItem::new("Halo CES", icon) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Gagal memuat System Tray: {:?}", e);
            eprintln!("Jika Anda menggunakan Linux tanpa UI atau WSL, Tray tidak akan muncul.");
            // Agar tidak langsung keluar, kita loop saja
            loop {
                thread::sleep(Duration::from_secs(10));
            }
        }
    };

    let _ = tray.add_label("Local Posture Detector");

    let (tx, rx) = mpsc::channel();

    let running_quit = running.clone();
    let tx_quit = tx.clone();
    let _ = tray.add_menu_item("Keluar", move || {
        println!("Menerima sinyal keluar dari Tray...");
        running_quit.store(false, Ordering::Relaxed);
        let _ = tx_quit.send(());
    });

    println!("System Tray berjalan. Silakan cek ikon di layar pojok atas Anda.");

    // Blokir main thread sampai tombol "Keluar" di-klik
    rx.recv().unwrap();
    println!("Aplikasi ditutup.");

    Ok(())
}