// src/api.rs — HTTP client for AX-Connect REST API
//
// Uses ureq (blocking, pure Rust) — no async runtime, no web framework.
// All requests are synchronous and complete before returning to the caller.

use serde_json::{json, Value};
use crate::session;

pub struct ApiClient {
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    fn url(&self, endpoint: &str) -> String {
        format!("{}{}", self.base_url, endpoint)
    }

    /// Build a ureq request with auth header injected if a session exists.
    fn agent() -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(10))
            .build()
    }

    fn inject_auth(req: ureq::Request) -> ureq::Request {
        if let Some(token) = session::token() {
            req.set("Authorization", &format!("Bearer {}", token))
        } else {
            req
        }
    }

    fn get_json(&self, endpoint: &str) -> Result<Value, String> {
        let req = Self::inject_auth(
            Self::agent()
                .get(&self.url(endpoint))
                .set("Accept", "application/json"),
        );
        Self::handle_response(req.call())
    }

    fn post_json(&self, endpoint: &str, body: Value) -> Result<Value, String> {
        let req = Self::inject_auth(
            Self::agent()
                .post(&self.url(endpoint))
                .set("Content-Type", "application/json")
                .set("Accept", "application/json"),
        );
        Self::handle_response(req.send_json(body))
    }

    fn handle_response(result: Result<ureq::Response, ureq::Error>) -> Result<Value, String> {
        match result {
            Ok(resp) => {
                let text = resp.into_string().map_err(|e| e.to_string())?;
                if text.is_empty() {
                    return Ok(json!({}));
                }
                serde_json::from_str(&text).map_err(|e| e.to_string())
            }
            Err(ureq::Error::Status(401, _)) => Err(
                "Unauthorized — session expired. Restart the terminal to log in again.".to_string(),
            ),
            Err(ureq::Error::Status(403, _)) => Err(
                "Access denied — your role cannot perform this action.".to_string(),
            ),
            Err(ureq::Error::Status(code, resp)) => {
                let msg = resp
                    .into_string()
                    .ok()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .and_then(|v| {
                        v["error"].as_str().map(|s| s.to_string())
                            .or_else(|| v["message"].as_str().map(|s| s.to_string()))
                    })
                    .unwrap_or_else(|| format!("HTTP {}", code));
                Err(msg)
            }
            Err(ureq::Error::Transport(t)) => {
                let msg = t.to_string();
                // Match both POSIX "Connection refused" and Windows os error 10061
                if msg.contains("Connection refused")
                    || msg.contains("Os {")
                    || msg.contains("actively refused")
                    || msg.contains("os error 10061")
                {
                    Err("Cannot reach AX-Connect. Is the server running on the configured port?".to_string())
                } else if msg.contains("timed out") {
                    Err("Request timed out after 10 seconds.".to_string())
                } else {
                    // Strip the noisy URL prefix ureq prepends (e.g. "http://…: Connection …")
                    let clean = if let Some(pos) = msg.find(": ") {
                        msg[pos + 2..].to_string()
                    } else {
                        msg
                    };
                    Err(clean)
                }
            }
        }
    }

    // ── Auth ────────────────────────────────────────────────────────────────

    pub fn login(&self, email: &str, password: &str) -> Result<Value, String> {
        self.post_json(
            "/api/platform/auth/login",
            json!({ "email": email, "password": password }),
        )
    }

    #[allow(dead_code)]
    pub fn logout(&self) -> Result<Value, String> {
        self.post_json("/api/platform/auth/logout", json!({}))
    }

    // ── Tenants ─────────────────────────────────────────────────────────────

    pub fn list_tenants(&self) -> Result<Value, String> {
        self.get_json("/api/platform/restaurants")
    }

    pub fn get_tenant(&self, id: u64) -> Result<Value, String> {
        // GET /api/platform/restaurants/:id returns
        // { "restaurant": {...}, "settings": {...}, "branches": [...], ... }
        // Unwrap the nested object so callers get a flat restaurant value.
        let resp = self.get_json(&format!("/api/platform/restaurants/{}", id))?;
        if resp["restaurant"].is_object() {
            Ok(resp["restaurant"].clone())
        } else {
            Ok(resp) // flat shape — forward as-is
        }
    }

    pub fn create_tenant(&self, name: &str) -> Result<Value, String> {
        self.post_json(
            "/api/platform/restaurants",
            json!({ "name": name, "slug": name }),
        )
    }

    pub fn terminate_tenant(&self, id: u64, confirm_slug: &str) -> Result<Value, String> {
        self.post_json(
            &format!("/api/platform/restaurants/{}/terminate", id),
            json!({ "confirmSlug": confirm_slug }),
        )
    }

    // ── System ──────────────────────────────────────────────────────────────

    pub fn health(&self) -> Result<Value, String> {
        self.get_json("/health")
    }

    /// Detailed health — requires platform auth. Returns memory, dbTime, env.
    pub fn detailed_health(&self) -> Result<Value, String> {
        self.get_json("/monitor/health/detailed")
    }

    // ── Audit ───────────────────────────────────────────────────────────────

    /// Fire-and-forget audit log.  Spawns a background thread and returns
    /// immediately.  Never blocks, never panics — even if the endpoint is down.
    pub fn audit_log(&self, action: &str, status: &str) {
        let url    = self.url("/api/audit/log");
        let token  = session::token();
        let action = action.to_string();
        let status = status.to_string();

        std::thread::spawn(move || {
            let mut req = ureq::post(&url)
                .set("Content-Type", "application/json")
                .set("Accept",       "application/json");
            if let Some(t) = token {
                req = req.set("Authorization", &format!("Bearer {}", t));
            }
            let body = json!({ "action": action, "status": status });
            let _ = req.send_json(body); // swallow every error silently
        });
    }
}
