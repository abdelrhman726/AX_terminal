'use strict';

/**
 * In-memory session store.
 * Holds the platform JWT for the lifetime of the terminal process.
 */
let _token = null;
let _user  = null;

function setToken(token, user) {
  _token = token;
  _user  = user || null;
}

function getToken() {
  return _token;
}

function getUser() {
  return _user;
}

function isAuthenticated() {
  return !!_token;
}

function clear() {
  _token = null;
  _user  = null;
}

module.exports = { setToken, getToken, getUser, isAuthenticated, clear };
