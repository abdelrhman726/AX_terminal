'use strict';

const chalk      = require('chalk');
const ora        = require('ora');
const apiService = require('../../services/api.service.js');

function validate(/* args */) {}

async function execute(/* args */) {
  const spinner = ora({ text: 'Fetching tenants...', color: 'white' }).start();

  let raw;
  try {
    raw = await apiService.listTenants();
    spinner.succeed(chalk.hex('#4CAF50')('Tenant data retrieved'));
  } catch (err) {
    spinner.fail(chalk.hex('#F44336')('Failed to fetch tenants'));
    throw err;
  }

  // AX-Connect returns { restaurants: [...] } or plain array
  const tenants = Array.isArray(raw)
    ? raw
    : (Array.isArray(raw.restaurants) ? raw.restaurants : []);

  if (tenants.length === 0) {
    console.log(chalk.hex('#A0A0A0')('\nNo tenants found.\n'));
    return { success: true };
  }

  console.log('');
  renderTable(tenants);
  return { success: true };
}

function renderTable(tenants) {
  const C = chalk;
  const h = C.hex('#EAEAEA');
  const g = C.hex('#A0A0A0');

  // Normalise fields from real AX-Connect tenant objects
  const rows = tenants.map((t) => ({
    id:     String(t.id),
    slug:   t.slug   || '—',
    name:   t.name   || '—',
    status: t.status || (t.isActive ? 'active' : 'inactive'),
    type:   t.businessType || '—'
  }));

  const idW   = Math.max(2,  ...rows.map((r) => r.id.length));
  const slugW = Math.max(4,  ...rows.map((r) => r.slug.length));
  const nameW = Math.max(4,  ...rows.map((r) => r.name.length));
  const statW = Math.max(6,  ...rows.map((r) => r.status.length));
  const typeW = Math.max(4,  ...rows.map((r) => r.type.length));

  const pad = (s, w) => String(s).padEnd(w);

  const line = (widths, left, mid, right, fill) =>
    left + widths.map((w) => fill.repeat(w + 2)).join(mid) + right;

  const widths = [idW, slugW, nameW, statW, typeW];

  console.log(g(line(widths, '┌', '┬', '┐', '─')));
  console.log(
    `│ ${h(pad('ID', idW))} │ ${h(pad('Slug', slugW))} │ ${h(pad('Name', nameW))} │ ${h(pad('Status', statW))} │ ${h(pad('Type', typeW))} │`
  );
  console.log(g(line(widths, '├', '┼', '┤', '─')));

  for (const row of rows) {
    const statusColor =
      row.status === 'active'     ? C.hex('#4CAF50') :
      row.status === 'suspended'  ? C.hex('#FF9800') :
                                    C.hex('#F44336');

    console.log(
      `│ ${h(pad(row.id, idW))} ` +
      `│ ${h(pad(row.slug, slugW))} ` +
      `│ ${h(pad(row.name, nameW))} ` +
      `│ ${statusColor(pad(row.status, statW))} ` +
      `│ ${h(pad(row.type, typeW))} │`
    );
  }

  console.log(g(line(widths, '└', '┴', '┘', '─')));
  console.log('');
}

module.exports = { validate, execute };
