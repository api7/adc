import { Command } from 'commander';

import { BackendCommand } from './helper';
import { BackendOptions } from './typing';

export const DevCommand = new BackendCommand<BackendOptions>(
  'dev',
  'Only for dev',
).handle(async (opts) => {
  console.log(opts);
});
