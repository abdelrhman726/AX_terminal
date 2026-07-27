// src/session.rs — in-memory JWT session store
//
// The JWT is never written to disk. It lives only for the duration of the
// running process. Thread-safe via a Mutex, though the terminal is single-threaded.

use std::sync::{Mutex, OnceLock};

struct SessionData {
    token: Option<String>,
    email: Option<String>,
    role:  Option<String>,   // "admin" | "operator" — derived from JWT payload
}

static SESSION: OnceLock<Mutex<SessionData>> = OnceLock::new();

fn get() -> &'static Mutex<SessionData> {
    SESSION.get_or_init(|| {
        Mutex::new(SessionData {
            token: None,
            email: None,
            role:  None,
        })
    })
}

pub fn set(token: String, email: Option<String>, role: Option<String>) {
    let mut s = get().lock().unwrap();
    s.token = Some(token);
    s.email = email;
    s.role  = role;
}

pub fn token() -> Option<String> {
    get().lock().unwrap().token.clone()
}

pub fn email() -> Option<String> {
    get().lock().unwrap().email.clone()
}

pub fn is_authenticated() -> bool {
    get().lock().unwrap().token.is_some()
}

/// Returns the authenticated user's role.
/// Defaults to "operator" — the least-privileged role — when no role is stored.
pub fn role() -> String {
    get().lock().unwrap()
        .role.clone()
        .unwrap_or_else(|| "operator".to_string())
}

#[allow(dead_code)]
pub fn clear() {
    let mut s = get().lock().unwrap();
    s.token = None;
    s.email = None;
    s.role  = None;
}
