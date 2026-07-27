'use strict';

const chalk      = require('chalk');
const apiService = require('../../services/api.service.js');
const session    = require('../../services/session.js');

function validate(/* args */) {}

async function execute(args, context) {
  // context.email / context.password come from the interactive startup prompt
  const email    = args.email    || (context && context.email);
  const password = args.password || (context && context.password);

  if (!email || !password) {
    throw new Error('Login requires --email and --password');
  }

  const result = await apiService.login(email, password);

  if (!result || !result.accessToken) {
    throw new Error('Login failed — no access token in response');
  }

  session.setToken(result.accessToken, result.user || null);

  const user = result.user;
  if (user) {
    console.log(chalk.hex('#4CAF50')(`✔ Logged in as ${user.email || email} (${user.role || 'platform'})`));
  } else {
    console.log(chalk.hex('#4CAF50')('✔ Login successful'));
  }

  return { success: true };
}

module.exports = { validate, execute };
