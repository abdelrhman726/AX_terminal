'use strict';

/**
 * Lightweight .env loader — no external dependencies.
 *
 * Reads AX_TERMINAL/.env from the directory next to the executable
 * (or the project root when running with `node cli/index.js`).
 *
 * Supported variables:
 *   AX_API_URL    Base URL of the AX-Connect REST API  (default: http://localhost:4000)
 *   AX_API_TOKEN  Pre-authenticated platform JWT — skips login prompt (optional)
 */

const fs   = require('fs');
const path = require('path');

function loadEnv() {
  // When packaged as .exe, __dirname is the snapshot root.
  // The .env file lives next to the .exe on disk — use process.execPath for that.
  const execDir    = path.dirname(process.execPath);
  const projectDir = path.resolve(__dirname, '..');

  const candidates = [
    path.join(execDir, '.env'),
    path.join(projectDir, '.env')
  ];

  for (const envPath of candidates) {
    if (fs.existsSync(envPath)) {
      const lines = fs.readFileSync(envPath, 'utf8').split('\n');
      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('#')) continue;
        const eqIdx = trimmed.indexOf('=');
        if (eqIdx === -1) continue;
        const key   = trimmed.slice(0, eqIdx).trim();
        const value = trimmed.slice(eqIdx + 1).trim().replace(/^["']|["']$/g, '');
        if (key && !(key in process.env)) {
          process.env[key] = value;
        }
      }
      break;
    }
  }
}

loadEnv();

module.exports = {
  AX_API_URL:   process.env.AX_API_URL   || 'http://localhost:4000',
  AX_API_TOKEN: process.env.AX_API_TOKEN || ''
};
