'use strict';

module.exports = {
  'platform:login': {
    handler:     'platform/login',
    permissions: ['admin', 'operator'],
    risk:        'low',
    description: 'Log in to AX-Connect platform',
    skipAuth:    true   // allowed before session is established
  },

  'tenant:list': {
    handler:     'tenant/list',
    permissions: ['admin', 'operator'],
    risk:        'low',
    description: 'List all tenants'
  },

  'tenant:delete': {
    handler:     'tenant/delete',
    permissions: ['admin'],
    risk:        'high',
    confirm:     true,
    description: 'Permanently terminate a tenant'
  },

  'system:health': {
    handler:     'system/health',
    permissions: ['admin', 'operator'],
    risk:        'low',
    description: 'Check system health status'
  }
};
