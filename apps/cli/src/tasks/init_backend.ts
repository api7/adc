import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

import { loadBackend } from '../command/utils';

export const InitializeBackendTask = (
  type: string,
  opts: Omit<ADCSDK.BackendOptions, 'logger'>,
): ListrTask => ({
  task: async (ctx) => {
    ctx.backend = loadBackend(type, { ...opts });
  },
});
