import chalk from 'chalk';

import { BackendCommand } from './helper';
import type { BackendOptions } from './typing';
import { loadBackend } from './utils';

type PingOptions = BackendOptions;

export const PingCommand = new BackendCommand<PingOptions>(
  'ping',
  'check connectivity with the backend',
  'Check connectivity with the configured backend.',
)
  .addExamples([
    {
      title: 'Check connectivity with the configured backend',
      command: 'adc ping',
    },
    {
      title: 'Check connectivity with the specified backend',
      command: 'adc ping --backend apisix --server http://192.168.1.21:9180',
    },
  ])
  .handle(async (opts) => {
    const backend = loadBackend(opts.backend, opts);

    try {
      await backend.ping();
      console.log(
        chalk.green(`Connected to the "${opts.backend}" backend successfully!`),
      );
    } catch (err) {
      console.log(
        chalk.red(`Unable to connect to the "${opts.backend}" backend. ${err}`),
      );
      if (opts.verbose === 2) console.log(err);
      process.exit(1);
    }
  });
