use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Baseline {
    pub average_neck_angle: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub baseline: Option<Baseline>,
    pub debounce_threshold_frames: u32,
    pub tolerance_angle: f32,
    #[serde(default = "default_calibration_frames")]
    pub calibration_frames: usize,
    #[serde(default = "default_monitoring_interval")]
    pub monitoring_interval_secs: u64,
    #[serde(default = "default_model_input_size")]
    pub model_input_size: u32,
}

fn default_calibration_frames() -> usize { 5 }
fn default_monitoring_interval() -> u64 { 2 }
fn default_model_input_size() -> u32 { 192 }

/// Resolve path relatif ke lokasi executable, bukan working directory.
pub fn get_app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn resolve_path(filename: &str) -> String {
    get_app_dir().join(filename).to_string_lossy().to_string()
}

impl Config {
    pub fn load_or_default() -> Self {
        let path = resolve_path("config.json");
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
        }
        Self {
            baseline: None,
            debounce_threshold_frames: 3,
            tolerance_angle: 15.0,
            calibration_frames: default_calibration_frames(),
            monitoring_interval_secs: default_monitoring_interval(),
            model_input_size: default_model_input_size(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = resolve_path("config.json");
        let temp_path = resolve_path("config.json.tmp");
        
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            
        fs::write(&temp_path, data)?;
        fs::rename(temp_path, path)?;
        
        Ok(())
    }
}