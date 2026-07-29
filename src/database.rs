use rusqlite::{Connection, Result};
use chrono::Local;
use std::path::PathBuf;

/// Resolve path database relatif ke lokasi executable, bukan working directory.
fn get_db_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("halo_ces.db")
}

pub fn init_db() -> Result<()> {
    let conn = Connection::open(get_db_path())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS posture_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL
        )",
        (),
    )?;

    Ok(())
}

pub fn log_event(event_type: &str) {
    let db_path = get_db_path();
    if let Ok(conn) = Connection::open(&db_path) {
        let now = Local::now().to_rfc3339();
        if let Err(e) = conn.execute(
            "INSERT INTO posture_logs (event_type, timestamp) VALUES (?1, ?2)",
            (event_type, now),
        ) {
            eprintln!("Gagal menyimpan event ke database: {}", e);
        }
    } else {
        eprintln!("Gagal membuka database untuk logging event: {:?}", db_path);
    }
}