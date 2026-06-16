import * as ADCSDK from '@api7/adc-sdk';
import { readFile } from 'fs/promises';
import YAML from 'js-yaml';

import type { TaskContext } from '../command/diff.command';

export const ExperimentalRemoteStateFileTask = (file: string) => ({
  task: async (ctx: TaskContext) => {
    const fileContent =
      (await readFile(file, {
        encoding: 'utf-8',
      })) ?? '';
    ctx.remote = YAML.load(fileContent) as ADCSDK.Configuration;
  },
});
