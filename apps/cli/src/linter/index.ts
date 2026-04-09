import { Configuration } from '@api7/adc-sdk';
import { ConfigurationSchema } from '@api7/adc-sdk/schema';
import { z } from 'zod';

import { validatePlugins } from './plugin-validator';
import type { LintError, LintOptions, LintResult } from './types';

export type { LintError, LintOptions, LintResult } from './types';

const zodIssuesToLintErrors = (issues: z.ZodIssue[]): LintError[] =>
  issues.map(({ path, message, code, ...rest }) => ({
    path,
    message,
    code,
    ...rest,
  }));

export const check = (
  config: Configuration,
  opts?: LintOptions,
): LintResult => {
  // Phase 1: core resource structure validation (local, no network)
  const result = ConfigurationSchema.safeParse(config);
  if (!result.success) {
    return {
      success: false,
      errors: zodIssuesToLintErrors(result.error.issues),
    };
  }

  // Phase 2: plugin config validation (requires plugin schemas from backend)
  if (opts?.pluginSchemas) {
    const pluginErrors = validatePlugins(
      result.data as Configuration,
      opts.pluginSchemas,
    );
    if (pluginErrors.length > 0) {
      return {
        success: false,
        errors: pluginErrors,
        data: result.data,
      };
    }
  }

  return {
    success: true,
    errors: [],
    data: result.data,
  };
};
