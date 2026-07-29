use crate::config::{Baseline, Config};
use crate::database;
use notify_rust::Notification;

#[derive(Debug, PartialEq)]
pub enum PostureState {
    Calibrating,
    Good,
    Bad,
    Paused,
}

pub struct StateEngine {
    pub state: PostureState,
    pub bad_posture_counter: u32,
    pub calibration_frames: Vec<f32>,
    pub config: Config,
}

impl StateEngine {
    pub fn new(config: Config) -> Self {
        let state = if config.baseline.is_some() {
            PostureState::Good
        } else {
            PostureState::Calibrating
        };

        Self {
            state,
            bad_posture_counter: 0,
            calibration_frames: Vec::new(),
            config,
        }
    }

    pub fn process_angle(&mut self, current_angle: f32) {
        if self.state == PostureState::Paused {
            return;
        }

        if self.state == PostureState::Calibrating {
            self.calibration_frames.push(current_angle);
            println!(
                "  [Kalibrasi] Menyerap frame {}/5...",
                self.calibration_frames.len()
            );

            if self.calibration_frames.len() >= 5 {
                let sum: f32 = self.calibration_frames.iter().sum();
                let avg = sum / self.calibration_frames.len() as f32;
                self.config.baseline = Some(Baseline {
                    average_neck_angle: avg,
                });
                if let Err(e) = self.config.save() {
                    eprintln!("  [Kalibrasi] Peringatan: Gagal menyimpan baseline ke config.json: {}", e);
                }
                self.state = PostureState::Good;
                println!(
                    "  [Kalibrasi] Selesai! Baseline sudut leher diatur ke {:.2}°",
                    avg
                );
            }
            return;
        }

        if let Some(ref baseline) = self.config.baseline {
            let deviation = (current_angle - baseline.average_neck_angle).abs();

            if deviation > self.config.tolerance_angle {
                self.bad_posture_counter += 1;

                if self.bad_posture_counter >= self.config.debounce_threshold_frames {
                    if self.state != PostureState::Bad {
                        self.state = PostureState::Bad;
                        println!("=> ⚠️  STATUS BERUBAH: POSTUR BURUK (Bungkuk terlalu lama)!");
                        database::log_event("BAD");

                        // Kirim Notifikasi OS
                        let _ = Notification::new()
                            .summary("Perbaiki Postur Anda!")
                            .body("Anda sudah membungkuk terlalu lama. Tegakkan punggung Anda.")
                            .icon("dialog-warning")
                            .show();
                    } else {
                        println!("=> ⚠️  STATUS: MASIH POSTUR BURUK");
                    }
                } else {
                    println!(
                        "=> Peringatan internal: Terdeteksi mulai bungkuk ({}/{} frame perlambatan)",
                        self.bad_posture_counter, self.config.debounce_threshold_frames
                    );
                }
            } else {
                if self.bad_posture_counter > 0 {
                    println!("=> Memperbaiki postur... Counter di-reset.");
                }
                self.bad_posture_counter = 0;

                if self.state != PostureState::Good {
                    self.state = PostureState::Good;
                    println!("=> ✅ STATUS BERUBAH: POSTUR KEMBALI BAIK");
                    database::log_event("GOOD");
                } else {
                    println!("=> ✅ STATUS: POSTUR BAIK");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Baseline, Config};

    fn test_config() -> Config {
        Config {
            baseline: None,
            debounce_threshold_frames: 3,
            tolerance_angle: 15.0,
            calibration_frames: 5,
            monitoring_interval_secs: 2,
            model_input_size: 192,
        }
    }

    fn calibrated_config() -> Config {
        Config {
            baseline: Some(Baseline {
                average_neck_angle: 15.0,
            }),
            debounce_threshold_frames: 3,
            tolerance_angle: 15.0,
            calibration_frames: 5,
            monitoring_interval_secs: 2,
            model_input_size: 192,
        }
    }

    #[test]
    fn test_kalibrasi_otomatis() {
        let mut engine = StateEngine::new(test_config());
        assert_eq!(engine.state, PostureState::Calibrating);

        for _ in 0..5 {
            engine.process_angle(15.0);
        }

        assert_eq!(engine.state, PostureState::Good);
        assert!(engine.config.baseline.is_some());
        let baseline = engine.config.baseline.as_ref().unwrap();
        assert!((baseline.average_neck_angle - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_postur_baik_dalam_toleransi() {
        let mut engine = StateEngine::new(calibrated_config());
        assert_eq!(engine.state, PostureState::Good);

        engine.process_angle(20.0); // deviasi 5° < toleransi 15°
        assert_eq!(engine.state, PostureState::Good);
    }

    #[test]
    fn test_debounce_postur_buruk() {
        let mut engine = StateEngine::new(calibrated_config());

        // Frame buruk 1 — belum berubah (debounce = 3)
        engine.process_angle(50.0);
        assert_eq!(engine.state, PostureState::Good);
        assert_eq!(engine.bad_posture_counter, 1);

        // Frame buruk 2
        engine.process_angle(50.0);
        assert_eq!(engine.state, PostureState::Good);
        assert_eq!(engine.bad_posture_counter, 2);

        // Frame buruk 3 — berubah ke Bad
        engine.process_angle(50.0);
        assert_eq!(engine.state, PostureState::Bad);
    }

    #[test]
    fn test_reset_counter_saat_membaik() {
        let mut engine = StateEngine::new(calibrated_config());

        engine.process_angle(50.0);
        engine.process_angle(50.0);
        assert_eq!(engine.bad_posture_counter, 2);

        engine.process_angle(15.0);
        assert_eq!(engine.bad_posture_counter, 0);
        assert_eq!(engine.state, PostureState::Good);
    }

    #[test]
    fn test_transisi_buruk_ke_baik() {
        let mut engine = StateEngine::new(calibrated_config());

        for _ in 0..3 {
            engine.process_angle(50.0);
        }
        assert_eq!(engine.state, PostureState::Bad);

        engine.process_angle(15.0);
        assert_eq!(engine.state, PostureState::Good);
        assert_eq!(engine.bad_posture_counter, 0);
    }

    #[test]
    fn test_paused_mengabaikan_input() {
        let mut engine = StateEngine::new(calibrated_config());
        engine.state = PostureState::Paused;

        engine.process_angle(50.0);
        assert_eq!(engine.state, PostureState::Paused);
        assert_eq!(engine.bad_posture_counter, 0);
    }
}