import chalk from 'chalk';

import { BackendCommand } from './helper';
import type { BackendOptions } from './typing';
import { loadBackend } from './utils';

type PingOptions = BackendOptions;

export const PingCommand = new BackendCommand<PingOptions>(
  'ping',
  'Verify connectivity with backend',
).handle(async (opts) => {
  const backend = loadBackend(opts.backend, opts);

  try {
    await backend.ping();
    console.log(chalk.green('Connected to backend successfully!'));
  } catch (err) {
    console.log(chalk.red(`Unable to connect to the backend, ${err}`));
    process.exit(1);
  }
});
