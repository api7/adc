import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

export const ValidateTask = (): ListrTask<{
  backend: ADCSDK.Backend;
  local: ADCSDK.Configuration;
}> => ({
  title: 'Validate configuration against backend',
  task: async (ctx) => {
    if (!ctx.backend.supportValidate) {
      throw new Error(
        'Validate is not supported by the current backend',
      );
    }

    const supported = await ctx.backend.supportValidate();
    if (!supported) {
      const version = await ctx.backend.version();
      throw new Error(
        `Validate is not supported by the current backend version (${version}). Please upgrade to a newer version.`,
      );
    }

    const result = await ctx.backend.validate!(ctx.local);
    if (!result.success) {
      const lines: string[] = [];
      if (result.errorMessage) {
        lines.push(result.errorMessage);
      }
      for (const e of result.errors) {
        const id = e.resource_id ? ` "${e.resource_id}"` : '';
        lines.push(`  - [${e.resource_type}${id}]: ${e.error}`);
      }
      const error = new Error(
        `Configuration validation failed:\n${lines.join('\n')}`,
      );
      error.stack = '';
      throw error;
    }
  },
});
