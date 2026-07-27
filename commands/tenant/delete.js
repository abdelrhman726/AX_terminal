'use strict';

const chalk      = require('chalk');
const ora        = require('ora');
const apiService = require('../../services/api.service.js');
const { isValidTenantId } = require('../../core/validation.js');

function validate(args) {
  if (!args.id) {
    throw new Error('Missing required argument: --id');
  }
  if (!isValidTenantId(args.id)) {
    throw new Error(`Invalid tenant ID: "${args.id}". Must be a positive integer.`);
  }
}

async function execute(args) {
  const tenantId = parseInt(args.id, 10);

  // Fetch real tenant details so we can show the slug and use it as confirmation
  const fetchSpinner = ora({ text: 'Fetching tenant details...', color: 'white' }).start();
  let tenant;
  try {
    tenant = await apiService.getTenant(tenantId);
    fetchSpinner.stop();
  } catch (err) {
    fetchSpinner.fail(chalk.hex('#F44336')('Could not fetch tenant details'));
    throw err;
  }

  if (!tenant) {
    throw new Error(`Tenant with ID ${tenantId} not found.`);
  }

  // Expose slug for the executor confirmation prompt
  args.slug = tenant.slug || tenant.name || String(tenantId);

  // Actually terminate — confirmSlug is the AX-Connect security gate
  const deleteSpinner = ora({ text: 'Terminating tenant...', color: 'white' }).start();
  try {
    await apiService.terminateTenant(tenantId, tenant.slug);
    deleteSpinner.succeed(chalk.hex('#4CAF50')(`Tenant "${tenant.slug}" (ID ${tenantId}) has been terminated`));
  } catch (err) {
    deleteSpinner.fail(chalk.hex('#F44336')('Termination failed'));
    throw err;
  }

  console.log('');
  console.log(chalk.hex('#A0A0A0')('The tenant and all associated data have been permanently removed.'));
  console.log('');

  return { success: true };
}

module.exports = { validate, execute };
