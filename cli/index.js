'use strict';

// Load .env before anything else so all modules see the env vars
const env = require('../config/env.js');

const readline = require('readline');

const ui               = require('./ui.js');
const { parseCommand }  = require('../core/parser.js');
const { execute }       = require('../core/executor.js');
const commandRegistry   = require('../config/commands.js');
const session           = require('../services/session.js');
const apiService        = require('../services/api.service.js');

// ─── Session configuration ─────────────────────────────────────────────────
// Phase 2: role will be derived from the JWT payload.
const SESSION_ROLE = 'admin';

// ─── readline setup ────────────────────────────────────────────────────────
const rl = readline.createInterface({
  input:     process.stdin,
  output:    process.stdout,
  terminal:  true,
  historySize: 100
});

// ─── Boot ──────────────────────────────────────────────────────────────────
start().catch((err) => {
  console.error('Fatal startup error:', err);
  process.exit(1);
});

async function start() {
  ui.clearScreen();
  ui.printLogo();
  await sleep(800);

  // Try pre-loaded token from .env
  if (env.AX_API_TOKEN) {
    session.setToken(env.AX_API_TOKEN, null);
    ui.printSuccess('Authenticated via AX_API_TOKEN');
    console.log('');
  } else {
    // Interactive login
    await interactiveLogin();
  }

  mainLoop();
}

/**
 * Prompt for platform credentials and exchange for a JWT.
 * Retries up to 3 times on failure.
 */
async function interactiveLogin() {
  const chalk = require('chalk');

  // Non-interactive environment — can't prompt for credentials
  if (!process.stdin.isTTY) {
    console.error(chalk.hex('#F44336')(
      '\nNon-interactive mode detected. Set AX_API_TOKEN in .env to skip login.\n'
    ));
    process.exit(1);
  }

  console.log(chalk.hex('#A0A0A0')('Platform login required.\n'));

  for (let attempt = 1; attempt <= 3; attempt++) {
    const email    = await ask('  Email:    ');
    const password = await askPassword('  Password: ');
    console.log('');

    const ora = require('ora');
    const spinner = ora({ text: 'Authenticating...', color: 'white' }).start();

    try {
      const result = await apiService.login(email, password);

      if (!result || !result.accessToken) {
        throw new Error('No access token returned');
      }

      session.setToken(result.accessToken, result.user || null);
      spinner.succeed(chalk.hex('#4CAF50')(
        `Logged in as ${result.user ? result.user.email : email}`
      ));
      console.log('');
      return;

    } catch (err) {
      spinner.fail(chalk.hex('#F44336')(
        `Authentication failed: ${err.message}`
      ));
      if (attempt < 3) {
        console.log(chalk.hex('#A0A0A0')(`  Attempt ${attempt}/3 — try again.\n`));
      } else {
        console.log(chalk.hex('#F44336')('\nToo many failed attempts. Exiting.\n'));
        process.exit(1);
      }
    }
  }
}

// ─── Main REPL loop ────────────────────────────────────────────────────────
function mainLoop() {
  rl.setPrompt(ui.prompt(SESSION_ROLE));
  rl.prompt();

  rl.on('line', async (line) => {
    const input = line.trim();

    if (!input) {
      rl.prompt();
      return;
    }

    // Built-in commands
    if (input === 'exit' || input === 'quit') {
      console.log(ui.C.secondary('\nGoodbye.\n'));
      rl.close();
      process.exit(0);
    }

    if (input === 'clear') {
      ui.clearScreen();
      ui.printLogo();
      rl.prompt();
      return;
    }

    if (input === 'help' || input === '--help') {
      ui.printHelp(commandRegistry);
      rl.prompt();
      return;
    }

    // Pause readline so spinners / confirmation prompts write cleanly
    rl.pause();

    try {
      ui.printInfo('Processing command...');
      const parsed = parseCommand(input);
      await execute(parsed, SESSION_ROLE, rl);
    } catch (err) {
      ui.printError(err.message);
    }

    rl.resume();
    rl.prompt();
  });

  rl.on('close', () => {
    console.log(ui.C.secondary('\nSession closed.\n'));
    process.exit(0);
  });

  rl.on('SIGINT', () => {
    console.log(ui.C.secondary('\n\nUse "exit" to quit.\n'));
    rl.prompt();
  });
}

// ─── Helpers ───────────────────────────────────────────────────────────────
function ask(promptText) {
  return new Promise((resolve) => {
    rl.question(promptText, (answer) => resolve(answer.trim()));
  });
}

function askPassword(promptText) {
  return new Promise((resolve) => {
    // Hide input on real TTY; fall back to visible input if not a TTY (CI/pipe)
    if (process.stdin.isTTY) {
      process.stdout.write(promptText);
      process.stdin.setRawMode(true);
      process.stdin.resume();

      let password = '';
      const onData = (ch) => {
        ch = ch.toString();
        if (ch === '\n' || ch === '\r' || ch === '\u0004') {
          process.stdin.setRawMode(false);
          process.stdin.pause();
          process.stdin.removeListener('data', onData);
          process.stdout.write('\n');
          resolve(password);
        } else if (ch === '\u0003') {
          // Ctrl+C
          process.stdout.write('\n');
          process.exit(0);
        } else if (ch === '\u007f') {
          // Backspace
          if (password.length > 0) {
            password = password.slice(0, -1);
            process.stdout.write('\b \b');
          }
        } else {
          password += ch;
          process.stdout.write('*');
        }
      };
      process.stdin.on('data', onData);
    } else {
      rl.question(promptText, (answer) => resolve(answer.trim()));
    }
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
