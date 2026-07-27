// src/main.rs — AX Terminal entry point
// Startup sequence → authentication → REPL loop

mod api;
mod audit_log;
mod commands;
mod config_store;
mod env;
mod parser;
mod session;
mod ui;

use api::ApiClient;
use rustyline::completion::{Completer, Pair};
use rustyline::config::Builder as RlBuilder;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Editor, Helper};
use std::borrow::Cow;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

// ── Prompt helper ─────────────────────────────────────────────────────────────
//
// Rustyline needs a PLAIN prompt string to calculate cursor offsets correctly.
// Passing a prompt that contains ANSI escape codes causes rustyline to count
// the invisible bytes as printable columns, which shifts every cursor position
// calculation — letting backspace eat the prompt text and making click-to-move
// land in the wrong place.
//
// The fix: pass plain text to readline(), implement Highlighter::highlight_prompt
// to return the colored version for display only.  Rustyline measures the plain
// copy and renders the colored copy — cursor math is always correct.

struct PromptHelper;

impl Completer for PromptHelper {
    type Candidate = Pair;
}
impl Hinter for PromptHelper {
    type Hint = String;
}
impl Validator for PromptHelper {}
impl Highlighter for PromptHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        // "ax (role) > "  →  dim "ax (" + bright role + dim ") > "
        let dim   = "\x1b[38;2;140;140;140m";
        let main  = "\x1b[38;2;234;234;234m";
        let reset = "\x1b[0m";
        if let (Some(l), Some(r)) = (prompt.find('('), prompt.rfind(')')) {
            let prefix = &prompt[..=l];   // "ax ("
            let role   = &prompt[l+1..r]; // "platform_manager"
            let suffix = &prompt[r..];    // ") > "
            return Cow::Owned(format!(
                "{dim}{prefix}{reset}{main}{role}{reset}{dim}{suffix}{reset}"
            ));
        }
        Cow::Owned(format!("{dim}{prompt}{reset}"))
    }
}
impl Helper for PromptHelper {}

fn main() {
    // ── 1. Load .env ────────────────────────────────────────────────────────
    let config = env::Config::load();
    let client = ApiClient::new(&config.api_url);

    // ── 2. Startup visual ───────────────────────────────────────────────────
    ui::clear_screen();
    ui::print_logo(&config.api_url);
    thread::sleep(Duration::from_millis(800));

    // ── 3. Authentication ───────────────────────────────────────────────────
    if !config.api_token.is_empty() {
        // Decode the JWT payload to extract the role without a network call.
        let role = role_from_jwt(&config.api_token);
        session::set(config.api_token.clone(), None, role);
        ui::print_success(&format!(
            "Authenticated via AX_API_TOKEN  (role: {})",
            session::role()
        ));
        println!();
    } else {
        if let Err(e) = interactive_login(&client) {
            eprintln!("{}", ui::error(&format!("Fatal: {}", e)));
            std::process::exit(1);
        }
    }

    // ── 4. REPL ─────────────────────────────────────────────────────────────
    repl(&client, &config.api_url);
}

// ── Helpers: extract role from an API login response ─────────────────────────

fn role_from_response(data: &serde_json::Value) -> Option<String> {
    data["user"]["role"].as_str().map(|s| s.to_string())
}

/// Decode the JWT payload segment (no crate needed — JWT is just base64url JSON).
/// Returns the `role` claim if present, without verifying the signature.
/// We trust the token's origin (it came from the server) and only use the role
/// for display and command gating — the server re-validates on every request.
fn role_from_jwt(token: &str) -> Option<String> {
    let payload_b64 = token.splitn(3, '.').nth(1)?;
    let decoded     = base64url_decode(payload_b64)?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    json["role"].as_str().map(|s| s.to_string())
}

fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    // base64url has no padding; add it back before decoding
    let pad = match s.len() % 4 {
        2 => "==",
        3 => "=",
        _ => "",
    };
    let b64: String = format!("{}{}", s, pad)
        .chars()
        .map(|c| match c { '-' => '+', '_' => '/', c => c })
        .collect();
    base64_decode_std(&b64)
}

fn base64_decode_std(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 { return None; }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut i = 0;
    while i < bytes.len() {
        let v0 = b64_val(bytes[i])?;
        let v1 = b64_val(bytes[i + 1])?;
        let v2 = b64_val(bytes[i + 2])?;
        let v3 = b64_val(bytes[i + 3])?;
        out.push((v0 << 2) | (v1 >> 4));
        if bytes[i + 2] != b'=' { out.push((v1 << 4) | (v2 >> 2)); }
        if bytes[i + 3] != b'=' { out.push((v2 << 6) | v3); }
        i += 4;
    }
    Some(out)
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        b'='        => Some(0),
        _           => None,
    }
}

// ── Interactive login ────────────────────────────────────────────────────────

fn interactive_login(client: &ApiClient) -> Result<(), String> {
    println!("{}", ui::secondary("  Platform login required.\n"));

    for attempt in 1u8..=3 {
        let email    = prompt_text("  Email:    ")?;
        let password = prompt_password("  Password: ")?;
        println!();

        let spinner = ui::Spinner::start("Authenticating");

        match client.login(&email, &password) {
            Ok(result) => {
                let token = result["accessToken"]
                    .as_str()
                    .ok_or("Login response missing accessToken")?
                    .to_string();

                let user_email = result["user"]["email"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| email.clone());

                let role = role_from_response(&result);

                session::set(token, Some(user_email.clone()), role);
                spinner.succeed(&format!("Logged in as {}  (role: {})", user_email, session::role()));
                println!();
                return Ok(());
            }
            Err(e) => {
                spinner.fail(&format!("Authentication failed: {}", e));
                if attempt < 3 {
                    println!(
                        "{}",
                        ui::secondary(&format!("  Attempt {}/3 — try again.\n", attempt))
                    );
                } else {
                    return Err("Too many failed login attempts.".to_string());
                }
            }
        }
    }

    Err("Login failed.".to_string())
}

fn prompt_text(label: &str) -> Result<String, String> {
    print!("{}", ui::secondary(label));
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf.trim().to_string())
}

fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(ui::secondary(label)).map_err(|e| e.to_string())
}

// ── REPL loop ────────────────────────────────────────────────────────────────

fn repl(client: &ApiClient, api_url: &str) {
    let config = RlBuilder::new()
        .history_ignore_dups(true)
        .expect("history_ignore_dups")
        .history_ignore_space(true)
        .max_history_size(500)
        .expect("max_history_size")
        .edit_mode(rustyline::EditMode::Emacs)
        .build();

    let mut rl: Editor<PromptHelper, DefaultHistory> =
        Editor::with_config(config).expect("Failed to initialise line editor");

    // Attach the prompt helper (handles prompt colorisation)
    rl.set_helper(Some(PromptHelper));

    // Persist history across sessions
    let history_path = history_file_path();
    if let Some(ref p) = history_path {
        let _ = rl.load_history(p);
    }

    loop {
        // Plain text — rustyline measures this for cursor/column maths.
        // PromptHelper::highlight_prompt() adds the colours for rendering.
        // Recomputed each iteration so re-login updates the role in the prompt.
        let plain_prompt = format!("ax ({}) > ", session::role());

        match rl.readline(&plain_prompt) {
            Ok(line) => {
                let input = line.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(&input);

                // Built-in commands handled before dispatch
                match input.as_str() {
                    "exit" | "quit" => {
                        println!("{}", ui::secondary("\n  Goodbye.\n"));
                        break;
                    }
                    "clear" => {
                        ui::clear_screen();
                        ui::print_logo(api_url);
                        continue;
                    }
                    "help" | "--help" => {
                        ui::print_help();
                        continue;
                    }
                    _ => {}
                }

                let parsed = parser::parse(&input);

                match commands::execute(&parsed, client) {
                    Ok(true)  => break,
                    Ok(false) => {}
                    Err(msg)  => ui::print_error(&msg),
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C — don't quit, just remind
                println!("{}", ui::secondary("\n  Use \"exit\" to quit.\n"));
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D or piped input ended
                println!("{}", ui::secondary("\n  Session closed.\n"));
                break;
            }
            Err(e) => {
                eprintln!("{}", ui::error(&format!("Input error: {}", e)));
                break;
            }
        }
    }

    if let Some(ref p) = history_path {
        let _ = rl.save_history(p);
    }
}

fn history_file_path() -> Option<std::path::PathBuf> {
    // Windows: %APPDATA%\ax-terminal\history
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = std::path::PathBuf::from(appdata).join("ax-terminal");
        let _ = std::fs::create_dir_all(&dir);
        return Some(dir.join("history"));
    }
    // Fallback: home directory
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".ax-terminal-history"))
}
