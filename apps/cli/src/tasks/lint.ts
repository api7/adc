import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

import { check } from '../linter';
import type { LintError } from '../linter';

const formatLintErrors = (errors: LintError[]): string =>
  errors
    .map(
      (e) =>
        `  - ${e.path.length > 0 ? e.path.join('.') + ': ' : ''}${e.message}`,
    )
    .join('\n');

export const LintTask = (): ListrTask<{
  local: ADCSDK.Configuration;
  pluginSchemas?: ADCSDK.PluginSchemaMap;
}> => ({
  title: 'Lint configuration',
  task: (ctx) => {
    const result = check(ctx.local, {
      pluginSchemas: ctx.pluginSchemas,
    });

    if (!result.success) {
      const err = `Lint configuration\nThe following errors were found in configuration:\n${formatLintErrors(result.errors)}`;
      const error = new Error(err);
      error.stack = '';
      throw error;
    }

    ctx.local = result.data as ADCSDK.Configuration;
  },
});
