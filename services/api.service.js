'use strict';

const http    = require('http');
const https   = require('https');
const env     = require('../config/env.js');
const session = require('./session.js');

/**
 * Core HTTP request helper.
 * Injects Bearer token from the active session automatically.
 */
function request(method, endpoint, body) {
  return new Promise((resolve, reject) => {
    const baseUrl = env.AX_API_URL.replace(/\/$/, '');
    const url     = new URL(baseUrl + endpoint);

    const isHttps   = url.protocol === 'https:';
    const transport = isHttps ? https : http;
    const payload   = body ? JSON.stringify(body) : null;

    const headers = {
      'Content-Type': 'application/json',
      'Accept':       'application/json'
    };

    // Attach JWT if available
    const token = session.getToken() || env.AX_API_TOKEN;
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

    if (payload) {
      headers['Content-Length'] = Buffer.byteLength(payload);
    }

    const options = {
      hostname: url.hostname,
      port:     url.port || (isHttps ? 443 : 80),
      path:     url.pathname + url.search,
      method,
      headers
    };

    const req = transport.request(options, (res) => {
      let data = '';
      res.on('data', (chunk) => { data += chunk; });
      res.on('end', () => {
        if (res.statusCode === 401) {
          return reject(new Error('Unauthorized — session expired or invalid. Please restart the terminal to log in again.'));
        }
        if (res.statusCode === 403) {
          return reject(new Error('Access denied — your role does not have permission for this action.'));
        }
        if (res.statusCode >= 400) {
          let message = `HTTP ${res.statusCode}`;
          try {
            const parsed = JSON.parse(data);
            message = parsed.message || parsed.error || message;
          } catch (_) {}
          return reject(new Error(message));
        }
        try {
          resolve(data ? JSON.parse(data) : {});
        } catch (_) {
          resolve(data);
        }
      });
    });

    req.on('error', (err) => {
      if (err.code === 'ECONNREFUSED') {
        reject(new Error(
          `Cannot reach AX-Connect at ${env.AX_API_URL}\n` +
          '  Ensure the server is running and AX_API_URL is correct in .env'
        ));
      } else {
        reject(err);
      }
    });

    req.setTimeout(10000, () => {
      req.destroy(new Error('Request timed out (10s)'));
    });

    if (payload) req.write(payload);
    req.end();
  });
}

class ApiService {
  // ─── Auth ──────────────────────────────────────────────────────────────────

  async login(email, password) {
    return request('POST', '/api/platform/auth/login', { email, password });
  }

  async logout() {
    return request('POST', '/api/platform/auth/logout');
  }

  // ─── Tenants ───────────────────────────────────────────────────────────────

  async listTenants() {
    return request('GET', '/api/platform/restaurants');
  }

  async getTenant(id) {
    return request('GET', `/api/platform/restaurants/${encodeURIComponent(id)}`);
  }

  /**
   * Permanently terminate a tenant.
   * AX-Connect requires the tenant's slug as confirmation in the body.
   */
  async terminateTenant(id, confirmSlug) {
    return request('POST', `/api/platform/restaurants/${encodeURIComponent(id)}/terminate`, {
      confirmSlug
    });
  }

  // ─── System ────────────────────────────────────────────────────────────────

  async getSystemHealth() {
    return request('GET', '/health');
  }

  async getDetailedHealth() {
    return request('GET', '/monitor/health/detailed');
  }
}

module.exports = new ApiService();
