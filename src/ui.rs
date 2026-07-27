// src/ui.rs — terminal output: colours, animated spinner, tables, prompts

use crossterm::style::{Color, Stylize};
use crossterm::terminal::{Clear, ClearType};
use crossterm::ExecutableCommand;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── Colour palette ────────────────────────────────────────────────────────────

pub fn primary(s: &str) -> String {
    s.with(Color::Rgb { r: 234, g: 234, b: 234 }).to_string()
}
pub fn secondary(s: &str) -> String {
    s.with(Color::Rgb { r: 140, g: 140, b: 140 }).to_string()
}
pub fn success(s: &str) -> String {
    s.with(Color::Rgb { r: 76, g: 175, b: 80 }).to_string()
}
pub fn error(s: &str) -> String {
    s.with(Color::Rgb { r: 244, g: 67, b: 54 }).to_string()
}
pub fn warning(s: &str) -> String {
    s.with(Color::Rgb { r: 255, g: 152, b: 0 }).to_string()
}
pub fn dim(s: &str) -> String {
    s.with(Color::Rgb { r: 90, g: 90, b: 90 }).to_string()
}
pub fn status_color(status: &str, s: &str) -> String {
    match status {
        "active" | "success" => success(s),
        "suspended"          => warning(s),
        _                    => error(s),
    }
}

// ── Screen ────────────────────────────────────────────────────────────────────

pub fn clear_screen() {
    let _ = stdout().execute(Clear(ClearType::All));
    print!("\x1B[H"); // cursor to top-left
    let _ = stdout().flush();
}

// ── Layout helpers ────────────────────────────────────────────────────────────

pub fn separator() -> String {
    secondary(&"━".repeat(36))
}

#[allow(dead_code)]
pub fn thin_rule() -> String {
    dim(&"─".repeat(36))
}

pub fn section_header(title: &str) -> String {
    format!("\n  {}\n  {}", primary(title), secondary(&"─".repeat(title.len())))
}

#[allow(dead_code)]
pub fn blank() {
    println!();
}

// ── Logo & branding ───────────────────────────────────────────────────────────

pub fn print_logo(api_url: &str) {
    let logo = concat!(
        "\n",
        "    ___   _  __\n",
        "   /   | | |/ /\n",
        "  / /| | |   / \n",
        " / ___ |/   |  \n",
        "/_/  |_/_/|_|  \n",
    );
    println!("{}", primary(logo));
    println!("  {}", primary("Ax Technology"));
    println!("  {}", separator());
    println!("  {}", secondary(&format!("Connected  {}", api_url)));
    println!();
}

// ── Feedback ──────────────────────────────────────────────────────────────────

pub fn print_success(msg: &str) {
    println!("  {}", success(&format!("✔  {}", msg)));
}
pub fn print_error(msg: &str) {
    println!("  {}", error(&format!("✖  Error: {}", msg)));
}
#[allow(dead_code)]
pub fn print_info(msg: &str) {
    println!("  {}", secondary(&format!("→  {}", msg)));
}
pub fn print_warning(msg: &str) {
    println!("  {}", warning(&format!("⚠  {}", msg)));
}

// ── Animated Spinner ──────────────────────────────────────────────────────────
//
// Runs on its own OS thread so it animates while the main thread waits for
// a network response. Thread is joined before the result line is printed,
// so the two never interleave.

pub struct Spinner {
    stop:    Arc<AtomicBool>,
    thread:  Option<std::thread::JoinHandle<()>>,
    msg_len: usize,
}

impl Spinner {
    pub fn start(message: &str) -> Self {
        let stop     = Arc::new(AtomicBool::new(false));
        let stop_ref = Arc::clone(&stop);
        let msg      = message.to_string();
        let msg_len  = msg.len() + 6; // frame + spaces

        let thread = std::thread::spawn(move || {
            let frames = ['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏'];
            let mut i  = 0usize;
            while !stop_ref.load(Ordering::Relaxed) {
                print!("\r  {}  {}  ",
                    secondary(&frames[i % frames.len()].to_string()),
                    secondary(&msg),
                );
                let _ = stdout().flush();
                i = i.wrapping_add(1);
                std::thread::sleep(Duration::from_millis(80));
            }
        });

        Spinner { stop, thread: Some(thread), msg_len }
    }

    pub fn succeed(self, msg: &str) {
        self.finish();
        println!("  {}", success(&format!("✔  {}", msg)));
    }

    pub fn fail(self, msg: &str) {
        self.finish();
        println!("  {}", error(&format!("✖  {}", msg)));
    }

    fn finish(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        // Erase the spinner line
        print!("\r{}\r", " ".repeat(self.msg_len + 8));
        let _ = stdout().flush();
    }
}

// Make sure thread is always stopped even on drop (e.g. on early return / ? propagation).
// Only clears the line when the thread was still running — prevents a double-clear
// when succeed()/fail() already called finish() before drop fires.
impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
            print!("\r{}\r", " ".repeat(self.msg_len + 8));
            let _ = stdout().flush();
        }
    }
}

// ── Help ──────────────────────────────────────────────────────────────────────

pub struct CommandHelp {
    pub name:        &'static str,
    pub alias:       &'static str,
    pub description: &'static str,
    pub roles:       &'static str,
    pub risk:        &'static str,
    pub usage:       &'static str,
}

pub const COMMANDS: &[CommandHelp] = &[
    CommandHelp {
        name: "platform:login", alias: "login",
        description: "Log in with a platform account",
        roles: "all",             risk: "low",
        usage: "platform:login --email <e> --password <p>",
    },
    CommandHelp {
        name: "tenant:list",    alias: "tlist",
        description: "List all tenants  (--output table|json)",
        roles: "admin, operator", risk: "low",
        usage: "tenant:list  [--output json]",
    },
    CommandHelp {
        name: "tenant:create",  alias: "tcreate",
        description: "Create a new tenant",
        roles: "admin only",      risk: "low",
        usage: "tenant:create --name <slug>",
    },
    CommandHelp {
        name: "tenant:use",     alias: "tuse",
        description: "Set the default tenant for this session",
        roles: "admin, operator", risk: "low",
        usage: "tenant:use --id <id>",
    },
    CommandHelp {
        name: "tenant:delete",  alias: "tdel",
        description: "Permanently terminate a tenant",
        roles: "admin only",      risk: "HIGH",
        usage: "tenant:delete --id <id>",
    },
    CommandHelp {
        name: "system:health",  alias: "health",
        description: "Check system and database health",
        roles: "admin, operator", risk: "low",
        usage: "system:health",
    },
    CommandHelp {
        name: "audit:log",      alias: "alog",
        description: "View local command history  (--user email  for other users)",
        roles: "all  (--user flag: admin only)", risk: "low",
        usage: "audit:log  [--user email]  [--output json]",
    },
];

pub fn print_help() {
    println!();
    println!("  {}", primary("Commands"));
    println!("  {}", separator());
    println!();

    for cmd in COMMANDS {
        let risk_tag = if cmd.risk == "HIGH" {
            format!("  {}", warning("⚠ HIGH RISK"))
        } else {
            String::new()
        };

        println!("  {}{}",
            primary(&format!("{:<20}", cmd.name)),
            risk_tag,
        );
        println!("  {}  {}",
            secondary(&format!("{:<20}", format!("  alias: {}", cmd.alias))),
            secondary(cmd.description),
        );
        println!("  {}  {}",
            " ".repeat(22),
            dim(&format!("Usage:  {}", cmd.usage)),
        );
        println!("  {}  {}",
            " ".repeat(22),
            dim(&format!("Roles:  {}", cmd.roles)),
        );
        println!();
    }

    println!("  {}", secondary(&"─".repeat(36)));
    println!("  {}", secondary("Built-in:  help    clear    exit"));
    println!();
    println!("  {}", dim("Keyboard shortcuts:"));
    println!("  {}", dim("  ↑ / ↓          Command history"));
    println!("  {}", dim("  Ctrl+A / ←     Start of line"));
    println!("  {}", dim("  Ctrl+E / →     End of line"));
    println!("  {}", dim("  Ctrl+W         Delete word back"));
    println!("  {}", dim("  Ctrl+K         Clear to end of line"));
    println!("  {}", dim("  Ctrl+U         Clear full line"));
    println!("  {}", dim("  Ctrl+R         Search history"));
    println!("  {}", dim("  Ctrl+Y         Paste last killed text"));
    println!("  {}", dim("  Ctrl+C         Cancel / interrupt"));
    println!();
}

// ── Table ─────────────────────────────────────────────────────────────────────

pub struct Table {
    pub headers:       Vec<String>,
    pub rows:          Vec<Vec<String>>,
    /// Column index that should receive status colouring (optional)
    pub status_column: Option<usize>,
}

impl Table {
    pub fn new(headers: Vec<&str>) -> Self {
        Table {
            headers:       headers.iter().map(|s| s.to_string()).collect(),
            rows:          Vec::new(),
            status_column: None,
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn with_status_column(mut self, col: usize) -> Self {
        self.status_column = Some(col);
        self
    }

    pub fn print(&self) {
        if self.rows.is_empty() {
            println!("  {}", secondary("(no results)"));
            println!();
            return;
        }

        let cols = self.headers.len();

        // Calculate column widths
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < cols {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        let border_line = |tl: &str, tm: &str, tr: &str, fill: &str| -> String {
            let parts: Vec<String> = widths.iter().map(|w| fill.repeat(w + 2)).collect();
            secondary(&format!("{}{}{}", tl, parts.join(tm), tr))
        };

        println!();
        println!("  {}", border_line("┌", "┬", "┐", "─"));

        // Header
        let hdr_cells: Vec<String> = self.headers.iter().enumerate()
            .map(|(i, h)| primary(&format!(" {:<w$} ", h, w = widths[i])))
            .collect();
        println!("  {}{}{}", secondary("│"), hdr_cells.join(&secondary("│")), secondary("│"));

        println!("  {}", border_line("├", "┼", "┤", "─"));

        // Rows — alternate subtle shading is handled by colour only
        for row in &self.rows {
            let cells: Vec<String> = row.iter().enumerate().map(|(i, cell)| {
                let w = widths.get(i).copied().unwrap_or(cell.len());
                if self.status_column == Some(i) {
                    format!(" {} ", status_color(cell, &format!("{:<w$}", cell, w = w)))
                } else {
                    primary(&format!(" {:<w$} ", cell, w = w))
                }
            }).collect();
            println!("  {}{}{}", secondary("│"), cells.join(&secondary("│")), secondary("│"));
        }

        println!("  {}", border_line("└", "┴", "┘", "─"));
        println!("  {}", dim(&format!("{} record{}", self.rows.len(),
            if self.rows.len() == 1 { "" } else { "s" })));
        println!();
    }
}
