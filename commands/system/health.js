'use strict';

const chalk      = require('chalk');
const ora        = require('ora');
const apiService = require('../../services/api.service.js');

function validate(/* args */) {}

async function execute(/* args */) {
  const spinner = ora({ text: 'Checking system health...', color: 'white' }).start();

  let health;
  try {
    health = await apiService.getSystemHealth();
    spinner.succeed(chalk.hex('#4CAF50')('Health check complete'));
  } catch (err) {
    spinner.fail(chalk.hex('#F44336')('Health check failed'));
    throw err;
  }

  console.log('');
  renderHealth(health);
  return { success: true };
}

function renderHealth(health) {
  const C   = chalk;
  const h   = C.hex('#EAEAEA');
  const g   = C.hex('#A0A0A0');
  const ok  = C.hex('#4CAF50');
  const err = C.hex('#F44336');
  const sep = g('━'.repeat(34));

  // AX-Connect /health returns: { status, uptime, database, services: { api, auth } }
  // or a simple { status: 'ok' } — handle both
  const status   = health.status   || 'UNKNOWN';
  const uptime   = health.uptime   || health.uptimeSeconds ? formatUptime(health.uptimeSeconds) : '—';
  const database = health.database || health.db || '—';
  const services = health.services || {};

  const statusColor = status.toUpperCase() === 'OK' ? ok : err;

  console.log(h(`System Status: ${statusColor('✔ ' + status.toUpperCase())}`));
  console.log(sep);

  if (uptime   !== '—') console.log(h(`Uptime:    ${uptime}`));
  if (database !== '—') console.log(h(`Database:  ${database}`));

  if (Object.keys(services).length > 0) {
    console.log(h('Services:'));
    for (const [name, state] of Object.entries(services)) {
      const sColor = String(state).toLowerCase() === 'healthy' ? ok : err;
      console.log(`  ${g('→')} ${h(name.toUpperCase().padEnd(6))} ${sColor(state)}`);
    }
  }

  // Show any extra fields AX-Connect includes
  const known = new Set(['status', 'uptime', 'uptimeSeconds', 'database', 'db', 'services']);
  for (const [key, val] of Object.entries(health)) {
    if (!known.has(key) && typeof val !== 'object') {
      console.log(h(`${key.padEnd(10)} ${val}`));
    }
  }

  console.log('');
}

function formatUptime(seconds) {
  if (!seconds) return '—';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h}h ${m}m`;
}

module.exports = { validate, execute };
