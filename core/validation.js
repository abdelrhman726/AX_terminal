'use strict';

/**
 * Validate a tenant ID.
 * Accepts positive integers only.
 */
function isValidTenantId(value) {
  if (value === undefined || value === null || value === '') return false;
  return /^\d+$/.test(String(value)) && parseInt(value, 10) > 0;
}

/**
 * Validate a tenant slug.
 * Accepts lowercase alphanumeric with hyphens; no leading/trailing hyphens.
 */
function isValidSlug(value) {
  if (typeof value !== 'string' || value.length === 0) return false;
  return /^[a-z0-9]+(-[a-z0-9]+)*$/.test(value);
}

module.exports = { isValidTenantId, isValidSlug };
