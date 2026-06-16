import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';
import { z } from 'zod';

import type { TaskContext } from '../command/diff.command';
import { check } from '../linter';

export const LintTask = (): ListrTask<TaskContext> => ({
  title: 'Lint configuration',
  task: (ctx) => {
    const result = check(ctx.local!);

    if (!result.success) {
      const err = `Lint configuration\nThe following errors were found in configuration:\n${z.prettifyError(result.error)}`;
      const error = new Error(err);
      error.stack = '';
      throw error;
    }

    ctx.local = result.data as ADCSDK.Configuration;
  },
});
