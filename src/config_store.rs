// src/config_store.rs — persistent CLI config at %USERPROFILE%\.ax\config.json
//
// Uses only std — no external dependencies.
// Default schema:  { "defaultTenant": null, "output": "table" }

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

// ── Paths ─────────────────────────────────────────────────────────────────────

fn ax_dir() -> Option<PathBuf> {
    // Windows: %USERPROFILE%\.ax    Unix fallback: $HOME/.ax
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|h| PathBuf::from(h).join(".ax"))
}

fn config_path() -> Option<PathBuf> {
    ax_dir().map(|d| d.join("config.json"))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Load config from disk.  Returns `{}` if the file does not exist — never
/// panics on a missing file.
pub fn load() -> Value {
    let path = match config_path() {
        Some(p) => p,
        None    => return json!({}),
    };
    if !path.exists() {
        return json!({});
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

/// Write the full config object to disk, creating `~/.ax/` if needed.
/// Includes a path-traversal guard — the write target must live inside `~/.ax/`.
pub fn save(data: &Value) -> Result<(), String> {
    let dir  = ax_dir().ok_or("Cannot determine home directory")?;
    let path = dir.join("config.json");

    // Create the directory first so we can canonicalise it.
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Path-traversal guard ─────────────────────────────────────────────────
    // Both `dir` and `path.parent()` must resolve to the same canonical dir.
    let canon_dir = fs::canonicalize(&dir).map_err(|e| e.to_string())?;
    // `path` itself doesn't exist yet (or does), but its parent must be `dir`.
    let parent = path.parent()
        .ok_or("Config path has no parent directory")?;
    let canon_parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    if canon_parent != canon_dir {
        return Err(
            "Path-traversal guard: refusing to write outside ~/.ax/".to_string()
        );
    }
    // ─────────────────────────────────────────────────────────────────────

    let json_str = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json_str).map_err(|e| e.to_string())
}

/// Read a single top-level key from config.  Returns `None` if the key does
/// not exist or the file is missing.
pub fn get(key: &str) -> Option<Value> {
    load().get(key).cloned()
}

/// Update a single top-level key and persist the whole config to disk.
pub fn set(key: &str, value: Value) -> Result<(), String> {
    let mut cfg = load();
    match cfg.as_object_mut() {
        Some(obj) => { obj.insert(key.to_string(), value); }
        None      => { cfg = json!({ key: value }); }
    }
    save(&cfg)
}
