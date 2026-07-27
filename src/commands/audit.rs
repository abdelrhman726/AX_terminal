// src/commands/audit.rs — audit:log command
//
// audit:log                    → show current user's own history
// audit:log --user email       → show any user's history (platform_owner / admin only)
// audit:log --output json      → raw JSON instead of table

use crate::audit_log;
use crate::commands::{current_role, Role};
use crate::parser::ParsedCommand;
use crate::session;
use crate::ui::{self, Table};

pub fn show(parsed: &ParsedCommand) -> Result<(), String> {
    let requested_user = parsed.get_str("user").map(|s| s.to_string());
    let output = parsed.get_str("output")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "table".to_string());

    if output != "table" && output != "json" {
        return Err(format!(
            "Invalid output format \"{}\": must be \"table\" or \"json\"",
            output
        ));
    }

    // Determine the email whose history we're showing
    let filter_email: String = match &requested_user {
        Some(target) => {
            // --user flag: admin-only
            if current_role() != Role::Admin {
                return Err(
                    "Access denied: only admin roles can view other users' audit logs.".to_string()
                );
            }
            target.clone()
        }
        None => {
            // Own history — need a session email
            session::email().ok_or_else(|| {
                "Cannot determine current user. Please log in first.".to_string()
            })?
        }
    };

    let entries = audit_log::read(Some(&filter_email));

    println!();
    println!("{}", ui::section_header(&format!("Audit Log  —  {}", filter_email)));

    if entries.is_empty() {
        println!("  {}", ui::secondary("(no audit entries found)"));
        println!();
        return Ok(());
    }

    // ── JSON output ───────────────────────────────────────────────────────────
    if output == "json" {
        let json_str = serde_json::to_string_pretty(&serde_json::Value::Array(entries))
            .map_err(|e| e.to_string())?;
        println!("{}", json_str);
        println!();
        return Ok(());
    }

    // ── Table output ──────────────────────────────────────────────────────────
    let mut table = Table::new(vec!["Timestamp", "Command", "Status"])
        .with_status_column(2);

    for entry in &entries {
        let ts  = entry["timestamp"].as_str().unwrap_or("—").to_string();
        let cmd = entry["command"].as_str().unwrap_or("—").to_string();
        let st  = entry["status"].as_str().unwrap_or("—").to_string();
        table.add_row(vec![ts, cmd, st]);
    }
    table.print();

    Ok(())
}
