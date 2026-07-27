'use strict';

const VALID_ROLES = ['admin', 'operator'];

/**
 * Check whether a role is permitted to run a command definition.
 * Throws immediately on unauthorized access — callers must not proceed.
 *
 * @param {string} role         - Current session role
 * @param {object} commandDef   - Entry from config/commands.js
 * @param {string} commandName  - Command name (for error messaging)
 */
function checkPermission(role, commandDef, commandName) {
  if (!VALID_ROLES.includes(role)) {
    throw new Error(`Unknown role: "${role}"`);
  }

  if (!Array.isArray(commandDef.permissions)) {
    throw new Error(`Command "${commandName}" has no permissions defined`);
  }

  if (!commandDef.permissions.includes(role)) {
    throw new Error(
      `Access denied: role "${role}" is not authorized to run "${commandName}"`
    );
  }
}

module.exports = { checkPermission };
