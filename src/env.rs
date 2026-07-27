// src/env.rs — .env file loader and config struct

use std::fs;
use std::path::PathBuf;

/// Global configuration derived from the .env file and environment variables.
pub struct Config {
    pub api_url:   String,
    pub api_token: String,
}

impl Config {
    pub fn load() -> Self {
        load_env_file();
        Config {
            api_url: std::env::var("AX_API_URL")
                .unwrap_or_else(|_| "http://localhost:4000".to_string()),
            api_token: std::env::var("AX_API_TOKEN").unwrap_or_default(),
        }
    }
}

/// Parse and inject a .env file into the process environment.
/// Searches next to the running executable first, then the current directory.
/// Never overwrites a variable that is already set.
fn load_env_file() {
    for path in candidate_paths() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                parse_and_inject(&content);
            }
            return;
        }
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    // next to the .exe (production)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join(".env"));
        }
    }
    // current working directory (dev mode)
    v.push(PathBuf::from(".env"));
    v
}

fn parse_and_inject(content: &str) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
            if !key.is_empty() && std::env::var(key).is_err() {
                // SAFETY: single-threaded at this point (called from main before threads spawn)
                unsafe { std::env::set_var(key, val) };
            }
        }
    }
}
