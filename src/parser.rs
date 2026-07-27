// src/parser.rs — tokenizer and argument extractor
//
// Converts a raw input string into a ParsedCommand.
// Supports:  command:name --key=value --key="quoted value" --bool-flag

use std::collections::HashMap;

#[derive(Debug)]
pub struct ParsedCommand {
    pub name: String,
    pub args: HashMap<String, ArgValue>,
}

#[derive(Debug, Clone)]
pub enum ArgValue {
    String(String),
    Bool(#[allow(dead_code)] bool),
}

impl ArgValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ArgValue::String(s) => Some(s.as_str()),
            ArgValue::Bool(_)   => None,
        }
    }
    #[allow(dead_code)]
    pub fn as_bool(&self) -> bool {
        match self {
            ArgValue::Bool(_b)  => true,
            ArgValue::String(s) => !s.is_empty(),
        }
    }
}

impl ParsedCommand {
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.args.get(key)?.as_str()
    }
}

pub fn parse(input: &str) -> ParsedCommand {
    let tokens = tokenize(input.trim());
    let name = tokens.first().cloned().unwrap_or_default();
    let mut args = HashMap::new();

    // Index-based so we can consume the token after a bare --key flag when
    // that next token is a value (does not start with --).
    let mut skip_next = false;
    for (i, token) in tokens.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        if let Some(body) = token.strip_prefix("--") {
            if let Some(eq) = body.find('=') {
                // --key=value  or  --key="quoted value"
                let key = sanitize_key(&body[..eq]);
                let val = strip_quotes(&body[eq + 1..]);
                if let Some(k) = key {
                    args.insert(k, ArgValue::String(val));
                }
            } else {
                // --key  — check if the next token is a bare value
                let next = tokens.get(i + 1);
                match next {
                    Some(nt) if !nt.starts_with("--") && !nt.is_empty() => {
                        // --key value
                        if let Some(k) = sanitize_key(body) {
                            args.insert(k, ArgValue::String(strip_quotes(nt)));
                        }
                        skip_next = true;
                    }
                    _ => {
                        // --flag  (boolean)
                        if let Some(k) = sanitize_key(body) {
                            args.insert(k, ArgValue::Bool(true));
                        }
                    }
                }
            }
        }
    }

    ParsedCommand { name, args }
}

/// Split on whitespace but keep quoted strings intact.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_double = false;
    let mut in_single = false;

    for ch in input.chars() {
        match ch {
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            ' ' if !in_double && !in_single => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn strip_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) ||
       (s.starts_with('\'') && s.ends_with('\''))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Only allow alphanumeric + hyphen keys to prevent prototype-style attacks.
fn sanitize_key(key: &str) -> Option<String> {
    if key.chars().all(|c| c.is_alphanumeric() || c == '-') && !key.is_empty() {
        Some(key.to_string())
    } else {
        None
    }
}
