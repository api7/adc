import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

import { loadBackend } from '../command/utils';
import { ListrOutputLogger } from '../utils/listr';

export const InitializeBackendTask = (
  type: string,
  opts: Omit<ADCSDK.BackendOptions, 'logger'>,
): ListrTask => ({
  task: async (ctx, task) => {
    ctx.backend = loadBackend(type, {
      ...opts,
      logger: new ListrOutputLogger(task),
    });
  },
});
