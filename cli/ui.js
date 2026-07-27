'use strict';

const chalk = require('chalk');

const C = {
  primary:   chalk.hex('#EAEAEA'),
  secondary: chalk.hex('#A0A0A0'),
  success:   chalk.hex('#4CAF50'),
  error:     chalk.hex('#F44336'),
  warning:   chalk.hex('#FF9800')
};

const LOGO = `
    ___   _  __
   /   | | |/ /
  / /| | |   /
 / ___ |/   |
/_/  |_/_/|_|
`;

const BRAND = 'Ax Technology';
const SEPARATOR = C.secondary('━'.repeat(34));

function clearScreen() {
  process.stdout.write('\x1Bc');
}

function printLogo() {
  const env = require('../config/env.js');
  console.log(C.primary(LOGO));
  console.log(C.primary(BRAND));
  console.log(SEPARATOR);
  console.log(C.secondary(`API  ${env.AX_API_URL}`));
  console.log('');
}

function prompt(role) {
  return C.secondary('ax ') + C.primary(`(${role})`) + C.secondary(' > ');
}

function printHelp(commandRegistry) {
  console.log('');
  console.log(C.primary('Available commands:'));
  console.log(SEPARATOR);

  for (const [name, def] of Object.entries(commandRegistry)) {
    const riskBadge = def.risk === 'high'
      ? C.warning(' [high-risk]')
      : '';
    const roles = C.secondary(`[${def.permissions.join(', ')}]`);
    console.log(`  ${C.primary(name.padEnd(20))} ${C.secondary(def.description)}${riskBadge} ${roles}`);
  }

  console.log('');
  console.log(C.secondary('Built-in commands: help, clear, exit'));
  console.log('');
}

function printSuccess(message) {
  console.log(C.success(`✔ ${message}`));
}

function printError(message) {
  console.log(C.error(`✖ Error: ${message}`));
}

function printInfo(message) {
  console.log(C.secondary(`→ ${message}`));
}

module.exports = {
  clearScreen,
  printLogo,
  prompt,
  printHelp,
  printSuccess,
  printError,
  printInfo,
  C
};
