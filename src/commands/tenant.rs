// src/commands/tenant.rs — tenant:list, tenant:create, tenant:use, tenant:delete

use crate::api::ApiClient;
use crate::config_store;
use crate::parser::ParsedCommand;
use crate::ui::{self, Spinner, Table};
use serde_json::Value;

// ── Shared validation ─────────────────────────────────────────────────────────

/// Valid slug: lowercase ASCII alphanumeric + hyphens, no leading/trailing hyphen.
fn is_valid_slug(s: &str) -> bool {
    if s.is_empty() || s.starts_with('-') || s.ends_with('-') {
        return false;
    }
    s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn parse_tenant_id(id_str: &str) -> Result<u64, String> {
    let id: u64 = id_str.parse()
        .map_err(|_| format!("Invalid tenant ID \"{}\": must be a positive integer", id_str))?;
    if id == 0 {
        return Err("Tenant ID must be greater than zero".to_string());
    }
    Ok(id)
}

// ── tenant:list ───────────────────────────────────────────────────────────────

pub fn list(parsed: &ParsedCommand, client: &ApiClient) -> Result<(), String> {
    // Validate --output if provided
    let output = parsed.get_str("output")
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Fall back to persisted preference, default "table"
            config_store::get("output")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "table".to_string())
        });

    if output != "table" && output != "json" {
        return Err(format!(
            "Invalid output format \"{}\": must be \"table\" or \"json\"",
            output
        ));
    }

    let spinner = Spinner::start("Fetching tenants");
    let raw     = client.list_tenants()?;
    spinner.succeed("Tenant data retrieved");

    // AX-Connect returns { restaurants: [...] } or a plain array
    let tenants: Vec<Value> = if raw.is_array() {
        raw.as_array().cloned().unwrap_or_default()
    } else {
        raw["restaurants"].as_array().cloned().unwrap_or_default()
    };

    if tenants.is_empty() {
        println!("{}", ui::secondary("  No tenants found."));
        return Ok(());
    }

    println!();

    if output == "json" {
        let json_str = serde_json::to_string_pretty(&Value::Array(tenants))
            .map_err(|e| e.to_string())?;
        println!("{}", json_str);
        return Ok(());
    }

    // ── Table output ─────────────────────────────────────────────────────────
    let rows: Vec<Vec<String>> = tenants.iter().map(|t| {
        let status = t["status"].as_str()
            .unwrap_or(if t["isActive"].as_bool().unwrap_or(false) { "active" } else { "inactive" })
            .to_string();

        vec![
            t["id"].to_string().trim_matches('"').to_string(),
            t["slug"].as_str().unwrap_or("—").to_string(),
            t["name"].as_str().unwrap_or("—").to_string(),
            status,
            t["businessType"].as_str().unwrap_or("—").to_string(),
        ]
    }).collect();

    let mut table = Table::new(vec!["ID", "Slug", "Name", "Status", "Type"])
        .with_status_column(3);
    for row in rows {
        table.add_row(row);
    }
    table.print();

    Ok(())
}

// ── tenant:create ─────────────────────────────────────────────────────────────

pub fn create(parsed: &ParsedCommand, client: &ApiClient) -> Result<(), String> {
    let name = parsed.get_str("name")
        .ok_or("Missing required argument: --name")?;

    if !is_valid_slug(name) {
        return Err(format!(
            "Invalid tenant name \"{}\": must be lowercase alphanumeric with hyphens (e.g. \"cafe-roma\")",
            name
        ));
    }

    let spinner = Spinner::start(&format!("Creating tenant \"{}\"", name));
    let result  = client.create_tenant(name)?;
    spinner.succeed("Tenant created");

    println!();
    println!("{}", ui::section_header("New Tenant"));

    let label_w = 8usize;
    let row = |label: &str, value: &str| {
        println!(
            "  {}  {}",
            ui::secondary(&format!("{:<w$}", label, w = label_w)),
            ui::primary(value)
        );
    };

    let id_str = result["id"].to_string();
    row("ID",     id_str.trim_matches('"'));
    row("Slug",   result["slug"].as_str().unwrap_or(name));
    row("Status", result["status"].as_str().unwrap_or("active"));

    println!();
    Ok(())
}

// ── tenant:use ────────────────────────────────────────────────────────────────

pub fn use_tenant(parsed: &ParsedCommand, client: &ApiClient) -> Result<(), String> {
    let id_str = parsed.get_str("id")
        .ok_or("Missing required argument: --id")?;
    let id = parse_tenant_id(id_str)?;

    // Confirm the tenant exists
    let spinner = Spinner::start("Verifying tenant");
    let tenant  = client.get_tenant(id)?;
    spinner.succeed("Tenant verified");

    let slug = tenant["slug"].as_str().unwrap_or("—");

    // Persist
    config_store::set("defaultTenant", serde_json::json!(id))?;

    println!();
    println!("  {}", ui::success(&format!("✔  Default tenant set to: {} (ID: {})", slug, id)));
    println!();
    Ok(())
}

// ── tenant:delete ─────────────────────────────────────────────────────────────

pub fn delete(parsed: &ParsedCommand, client: &ApiClient) -> Result<(), String> {
    let id_str = parsed.get_str("id")
        .ok_or("Missing required argument: --id")?;
    let id = parse_tenant_id(id_str)?;

    // Fetch the tenant to get its slug (needed for AX-Connect terminate call)
    let fetch  = Spinner::start("Fetching tenant details");
    let tenant = client.get_tenant(id)?;
    fetch.succeed("Tenant details retrieved");

    let slug = tenant["slug"].as_str()
        .ok_or("Could not read tenant slug from API response")?;
    let name = tenant["name"].as_str().unwrap_or(slug);

    println!("{}", ui::primary(&format!("  Tenant: {} ({})", name, slug)));
    println!();

    // The executor already got DELETE confirmation before calling here.
    let terminate = Spinner::start("Terminating tenant");
    client.terminate_tenant(id, slug)?;
    terminate.succeed(&format!("Tenant \"{}\" (ID {}) has been terminated", slug, id));

    println!();
    println!("{}", ui::secondary("  The tenant and all associated data have been permanently removed."));
    println!();
    Ok(())
}
