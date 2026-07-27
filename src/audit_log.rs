// src/audit_log.rs — local command-history log
//
// Every command execution is appended as a JSONL entry to:
//   Windows  → %APPDATA%\ax-terminal\audit.jsonl
//
// The file is never sent to the server.  The audit:log command reads it.
// All I/O errors are silently swallowed — audit must never crash the terminal.

use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::time::{SystemTime, UNIX_EPOCH};

// ── File path ─────────────────────────────────────────────────────────────────

fn log_file_path() -> Option<std::path::PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = std::path::PathBuf::from(appdata).join("ax-terminal");
        let _ = fs::create_dir_all(&dir);
        return Some(dir.join("audit.jsonl"));
    }
    // Fallback: home directory
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".ax-audit.jsonl"))
}

// ── ISO 8601 timestamp (no chrono crate) ─────────────────────────────────────

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn iso8601(unix_secs: u64) -> String {
    let mut days    = unix_secs / 86400;
    let time_of_day = unix_secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    // Resolve year
    let mut year = 1970u32;
    loop {
        let diy = if is_leap(year) { 366u64 } else { 365u64 };
        if days < diy { break; }
        days -= diy;
        year += 1;
    }

    // Resolve month / day
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for &dim in &month_days {
        if days < dim { break; }
        days -= dim;
        month += 1;
    }
    let day = days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, h, m, s
    )
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    iso8601(secs)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Append one audit entry.  Never blocks, never panics.
pub fn append(user: &str, role: &str, command: &str, status: &str) {
    let path = match log_file_path() {
        Some(p) => p,
        None    => return,
    };
    let entry = json!({
        "timestamp": now_iso8601(),
        "user":      user,
        "role":      role,
        "command":   command,
        "status":    status,
    });
    let line = match serde_json::to_string(&entry) {
        Ok(l)  => l,
        Err(_) => return,
    };
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", line);
    }
}

/// Read audit entries, optionally filtered by email (exact match, case-insensitive).
/// Returns entries in chronological order (oldest first).
pub fn read(filter_user: Option<&str>) -> Vec<Value> {
    let path = match log_file_path() {
        Some(p) => p,
        None    => return Vec::new(),
    };
    let file = match fs::File::open(&path) {
        Ok(f)  => f,
        Err(_) => return Vec::new(),
    };
    let filter_lower = filter_user.map(|u| u.to_lowercase());

    BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter_map(|l| serde_json::from_str::<Value>(&l).ok())
        .filter(|entry| {
            match &filter_lower {
                Some(target) => {
                    entry["user"]
                        .as_str()
                        .map(|u| u.to_lowercase() == *target)
                        .unwrap_or(false)
                }
                None => true,
            }
        })
        .collect()
}
