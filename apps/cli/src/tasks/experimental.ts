import { readFile } from 'fs/promises';
import YAML from 'js-yaml';
import { ConfigurationSchema } from '@api7/adc-sdk/schema';

import type { TaskContext } from '../command/diff.command';

export const ExperimentalRemoteStateFileTask = (file: string) => ({
  task: async (ctx: TaskContext) => {
    const fileContent =
      (await readFile(file, {
        encoding: 'utf-8',
      })) ?? '';
    const parsed = ConfigurationSchema.safeParse(YAML.load(fileContent));
    if (!parsed.success)
      throw new Error(`Invalid configuration in "${file}": ${parsed.error.message}`);
    ctx.remote = parsed.data;
  },
});
