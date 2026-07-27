'use strict';

const readline = require('readline');

const commandRegistry = require('../config/commands.js');
const aliases         = require('../config/aliases.js');
const session         = require('../services/session.js');
const { checkPermission } = require('./permissions.js');

/**
 * Static handler map — all command modules must be listed here.
 * Dynamic require(path) does not work inside a pkg-packaged binary.
 * When adding a new command: register it here AND in config/commands.js.
 */
const HANDLERS = {
  'platform/login': require('../commands/platform/login.js'),
  'tenant/list':    require('../commands/tenant/list.js'),
  'tenant/delete':  require('../commands/tenant/delete.js'),
  'system/health':  require('../commands/system/health.js')
};

/**
 * Execute a parsed command object under the given role.
 *
 * Flow:
 *   alias resolution → registry lookup → session check →
 *   permission check → argument validation →
 *   confirmation (high-risk) → handler execution
 */
async function execute(parsed, role, rl) {
  const { args } = parsed;
  let commandName = parsed.command;

  // 1. Alias resolution
  if (aliases[commandName]) {
    commandName = aliases[commandName];
  }

  // 2. Registry lookup
  const commandDef = commandRegistry[commandName];
  if (!commandDef) {
    throw new Error(`Unknown command: "${commandName}". Type "help" to see available commands.`);
  }

  // 3. Session check — require authenticated session unless command is skipAuth
  if (!commandDef.skipAuth && !session.isAuthenticated()) {
    throw new Error('Not logged in. Run "login" or set AX_API_TOKEN in .env');
  }

  // 4. Permission check
  checkPermission(role, commandDef, commandName);

  // 5. Handler lookup
  const handler = HANDLERS[commandDef.handler];
  if (!handler) {
    throw new Error(`Handler not found for "${commandName}": "${commandDef.handler}"`);
  }

  // 6. Argument validation
  if (typeof handler.validate === 'function') {
    handler.validate(args);
  }

  // 7. High-risk confirmation
  if (commandDef.risk === 'high' && commandDef.confirm === true) {
    const confirmed = await promptConfirmation(commandName, args, rl);
    if (!confirmed) {
      return { cancelled: true };
    }
  }

  // 8. Handler execution
  const result = await handler.execute(args);
  return result;
}

/**
 * Display a high-risk confirmation prompt.
 * Only proceeds if the user types exactly "DELETE" (case-sensitive).
 */
function promptConfirmation(commandName, args, rl) {
  const chalk     = require('chalk');
  const separator = chalk.hex('#A0A0A0')('━'.repeat(34));

  console.log('');
  console.log(chalk.hex('#FF9800')('⚠  HIGH RISK ACTION'));
  console.log(separator);
  console.log(chalk.hex('#EAEAEA')(`Command:    ${commandName}`));

  if (args.id)   console.log(chalk.hex('#EAEAEA')(`Tenant ID:  ${args.id}`));
  if (args.slug) console.log(chalk.hex('#EAEAEA')(`Slug:       ${args.slug}`));

  console.log('');
  console.log(chalk.hex('#F44336')('This action is IRREVERSIBLE.'));
  console.log('');

  return new Promise((resolve) => {
    rl.question(
      chalk.hex('#EAEAEA')('Type "DELETE" to confirm: '),
      (answer) => {
        if (answer === 'DELETE') {
          resolve(true);
        } else {
          console.log(chalk.hex('#A0A0A0')('\nAborted.'));
          resolve(false);
        }
      }
    );
  });
}

module.exports = { execute };
