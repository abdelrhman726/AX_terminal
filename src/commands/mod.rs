// src/commands/mod.rs — command registry, permissions, and executor

pub mod audit;
pub mod platform;
pub mod tenant;
pub mod system;

use crate::api::ApiClient;
use crate::audit_log;
use crate::parser::ParsedCommand;
use crate::session;
use crate::ui;
use std::io::{self, BufRead, Write};

// ── Role ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Admin,
    #[allow(dead_code)]
    Operator,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin    => write!(f, "admin"),
            Role::Operator => write!(f, "operator"),
        }
    }
}

/// Derive the current Role from the live session (defaults to Operator).
/// AX-Connect platform roles that map to Admin:
///   platform_owner, platform_manager  — full platform access
/// Everything else (platform_developer, restaurant_*, unknown) → Operator.
pub fn current_role() -> Role {
    match session::role().as_str() {
        "admin"
        | "platform_owner"
        | "platform_manager" => Role::Admin,
        _                    => Role::Operator,
    }
}

// ── Command definition ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    PlatformLogin,
    TenantList,
    TenantCreate,
    TenantUse,
    TenantDelete,
    SystemHealth,
    AuditLog,
}

struct CommandDef {
    kind:          CommandKind,
    requires_auth: bool,
    admin_only:    bool,
    high_risk:     bool,
}

// Static registry — includes aliases inline
const REGISTRY: &[(&str, CommandKind)] = &[
    ("platform:login", CommandKind::PlatformLogin),
    ("login",          CommandKind::PlatformLogin),
    ("tenant:list",    CommandKind::TenantList),
    ("tlist",          CommandKind::TenantList),
    ("tenant:create",  CommandKind::TenantCreate),
    ("tcreate",        CommandKind::TenantCreate),
    ("tenant:use",     CommandKind::TenantUse),
    ("tuse",           CommandKind::TenantUse),
    ("tenant:delete",  CommandKind::TenantDelete),
    ("tdel",           CommandKind::TenantDelete),
    ("system:health",  CommandKind::SystemHealth),
    ("health",         CommandKind::SystemHealth),
    ("audit:log",      CommandKind::AuditLog),
    ("alog",           CommandKind::AuditLog),
];

fn resolve(name: &str) -> Option<CommandDef> {
    let kind = REGISTRY.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| k.clone())?;

    let (requires_auth, admin_only, high_risk) = match &kind {
        CommandKind::PlatformLogin => (false, false, false),
        CommandKind::TenantList    => (true,  false, false),
        CommandKind::TenantCreate  => (true,  true,  false),
        CommandKind::TenantUse     => (true,  false, false),
        CommandKind::TenantDelete  => (true,  true,  true),
        CommandKind::SystemHealth  => (true,  false, false),
        // audit:log is auth-required; --user permission enforced inside the command
        CommandKind::AuditLog      => (true,  false, false),
    };

    Some(CommandDef { kind, requires_auth, admin_only, high_risk })
}

// ── Executor ──────────────────────────────────────────────────────────────────

/// Run a parsed command.
/// Returns `Ok(true)` if the terminal should exit, `Ok(false)` to continue.
pub fn execute(parsed: &ParsedCommand, client: &ApiClient) -> Result<bool, String> {
    let def = resolve(&parsed.name)
        .ok_or_else(|| format!(
            "Unknown command: \"{}\". Type \"help\" to see available commands.",
            parsed.name
        ))?;

    // Derive role dynamically from the current session
    let role = current_role();

    // Session check
    if def.requires_auth && !session::is_authenticated() {
        return Err("Not logged in. Run \"login\" or set AX_API_TOKEN in .env".to_string());
    }

    // Permission check
    if def.admin_only && role != Role::Admin {
        return Err(format!(
            "Access denied: \"{}\" requires admin role.",
            parsed.name
        ));
    }

    // High-risk confirmation
    if def.high_risk {
        if !confirm_high_risk(&parsed.name, parsed)? {
            println!("{}", ui::secondary("  Aborted."));
            return Ok(false);
        }
    }

    // Dispatch — capture result so we can audit-log before returning
    let is_login    = def.kind == CommandKind::PlatformLogin;
    let is_audit_cmd = def.kind == CommandKind::AuditLog;

    let result = match def.kind {
        CommandKind::PlatformLogin => platform::login(parsed, client),
        CommandKind::TenantList    => tenant::list(parsed, client),
        CommandKind::TenantCreate  => tenant::create(parsed, client),
        CommandKind::TenantUse     => tenant::use_tenant(parsed, client),
        CommandKind::TenantDelete  => tenant::delete(parsed, client),
        CommandKind::SystemHealth  => system::health(client),
        CommandKind::AuditLog      => audit::show(parsed),
    };

    // ── Post-execution audit logging ─────────────────────────────────────────
    // Skip for:
    //   • platform:login  — credentials were in flight; don't record before auth settles
    //   • audit:log       — reading the log shouldn't appear in the log (avoids clutter)
    if !is_login && !is_audit_cmd {
        let status = if result.is_ok() { "success" } else { "error" };
        let user   = session::email().unwrap_or_else(|| "unknown".to_string());
        let role   = session::role();

        // 1. Local JSONL — primary storage, always available offline
        audit_log::append(&user, &role, &parsed.name, status);

        // 2. Server-side fire-and-forget (Task 3 — non-blocking, swallows errors)
        client.audit_log(&parsed.name, status);
    }

    result?;
    Ok(false)
}

// ── High-risk confirmation prompt ─────────────────────────────────────────────

fn confirm_high_risk(command: &str, parsed: &ParsedCommand) -> Result<bool, String> {
    println!();
    ui::print_warning("HIGH RISK ACTION");
    println!("  {}", ui::separator());
    println!("  {}", ui::primary(&format!("Command:   {}", command)));
    if let Some(id) = parsed.get_str("id") {
        println!("  {}", ui::primary(&format!("Tenant ID: {}", id)));
    }
    if let Some(slug) = parsed.get_str("slug") {
        println!("  {}", ui::primary(&format!("Slug:      {}", slug)));
    }
    println!();
    println!("  {}", ui::error("This action is IRREVERSIBLE."));
    println!();
    print!("  {}", ui::primary("Type \"DELETE\" to confirm: "));
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).map_err(|e| e.to_string())?;

    Ok(line.trim() == "DELETE")
}
