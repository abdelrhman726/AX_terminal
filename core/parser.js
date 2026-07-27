'use strict';

/**
 * Parse a raw input string into a structured command + args object.
 *
 * Supported flag formats:
 *   --key=value
 *   --key="value with spaces"
 *   --key='value with spaces'
 *   --key=value\ with\ spaces   (not supported — use quotes)
 *
 * Returns: { command: string, args: object }
 */
function parseCommand(input) {
  if (typeof input !== 'string') {
    throw new Error('Input must be a string');
  }

  const trimmed = input.trim();
  if (!trimmed) {
    return { command: '', args: {} };
  }

  // Tokenize respecting quoted strings
  const tokens = tokenize(trimmed);
  if (tokens.length === 0) {
    return { command: '', args: {} };
  }

  const command = tokens[0];
  const args = {};

  for (let i = 1; i < tokens.length; i++) {
    const token = tokens[i];

    if (token.startsWith('--')) {
      const body = token.slice(2);
      const eqIdx = body.indexOf('=');

      if (eqIdx === -1) {
        // Boolean flag: --verbose
        const key = sanitizeKey(body);
        if (key) args[key] = true;
      } else {
        const key = sanitizeKey(body.slice(0, eqIdx));
        const raw = body.slice(eqIdx + 1);
        const value = stripQuotes(raw);
        if (key) args[key] = value;
      }
    }
    // Non-flag positional arguments are ignored per spec
  }

  return { command, args };
}

/**
 * Split input into tokens, preserving quoted strings as single tokens.
 * Supports both single and double quotes.
 */
function tokenize(input) {
  const tokens = [];
  let current = '';
  let inDouble = false;
  let inSingle = false;

  for (let i = 0; i < input.length; i++) {
    const ch = input[i];

    if (ch === '"' && !inSingle) {
      inDouble = !inDouble;
      current += ch;
    } else if (ch === "'" && !inDouble) {
      inSingle = !inSingle;
      current += ch;
    } else if (ch === ' ' && !inDouble && !inSingle) {
      if (current.length > 0) {
        tokens.push(current);
        current = '';
      }
    } else {
      current += ch;
    }
  }

  if (current.length > 0) {
    tokens.push(current);
  }

  return tokens;
}

/**
 * Strip surrounding single or double quotes from a value string.
 */
function stripQuotes(value) {
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1);
  }
  return value;
}

/**
 * Allow only alphanumeric keys and hyphens to prevent prototype pollution.
 */
function sanitizeKey(key) {
  if (/^[a-zA-Z0-9-]+$/.test(key)) {
    return key;
  }
  return null;
}

module.exports = { parseCommand };
