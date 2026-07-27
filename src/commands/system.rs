// src/commands/system.rs — system:health

use crate::api::ApiClient;
use crate::ui::{self, Spinner};

pub fn health(client: &ApiClient) -> Result<(), String> {
    let spinner = Spinner::start("Checking system health");

    // Prefer the detailed endpoint (requires platform auth we already have).
    // Falls back to basic /health if detailed returns an error.
    let data = client.detailed_health().or_else(|_| client.health())?;

    let is_ok = data["ok"].as_bool().unwrap_or(false);

    spinner.succeed(if is_ok { "Health check complete" } else { "Server is reporting issues" });

    println!();

    println!("{}", ui::section_header("System Health"));

    let label_w = 14;
    let row = |label: &str, value: &str| {
        println!(
            "  {}  {}",
            ui::secondary(&format!("{:<w$}", label, w = label_w)),
            ui::primary(value)
        );
    };
    let row_ok = |label: &str, value: &str, ok: bool| {
        let v = if ok { ui::success(value) } else { ui::error(value) };
        println!("  {}  {}", ui::secondary(&format!("{:<w$}", label, w = label_w)), v);
    };

    row_ok("Status", if is_ok { "ONLINE" } else { "DEGRADED" }, is_ok);

    if let Some(service) = data["service"].as_str().or_else(|| data["app"].as_str()) {
        row("Service", service);
    }
    if let Some(env) = data["environment"].as_str() {
        row("Environment", env);
    }

    println!("  {}", ui::secondary(&"─".repeat(36)));

    // ── Uptime ───────────────────────────────────────────────────────────────
    if let Some(secs) = data["uptimeSeconds"].as_u64() {
        row("Uptime", &format_uptime(secs));
    }

    // ── Database ─────────────────────────────────────────────────────────────
    if let Some(db_time) = data["dbTime"].as_str() {
        // dbTime is a timestamp string from SELECT NOW() — just show connected
        row_ok("Database", &format!("connected  ({})", &db_time[..19]), true);
    }

    // ── Memory ───────────────────────────────────────────────────────────────
    if let Some(mem) = data["memory"].as_object() {
        println!("  {}", ui::secondary(&"─".repeat(36)));

        let rss = mem.get("rss")
            .and_then(|v| v.as_u64())
            .map(|b| format!("{:.1} MB", b as f64 / 1_048_576.0))
            .unwrap_or_else(|| "—".to_string());

        let heap_used = mem.get("heapUsed").and_then(|v| v.as_u64()).unwrap_or(0);
        let heap_total = mem.get("heapTotal").and_then(|v| v.as_u64()).unwrap_or(0);
        let heap_str = if heap_total > 0 {
            format!("{:.1} / {:.1} MB",
                heap_used  as f64 / 1_048_576.0,
                heap_total as f64 / 1_048_576.0)
        } else {
            "—".to_string()
        };

        row("Memory RSS", &rss);
        row("Heap", &heap_str);
    }

    println!();
    Ok(())
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}
