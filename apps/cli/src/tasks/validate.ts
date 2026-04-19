import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';
import { lastValueFrom } from 'rxjs';

export const ValidateTask = (): ListrTask<{
  backend: ADCSDK.Backend;
  diff: ADCSDK.Event[];
}> => ({
  title: 'Validate configuration against backend',
  task: async (ctx) => {
    if (!ctx.backend.validate)
      throw new Error(`Validate is not supported by the current backend.`);

    const result = await lastValueFrom(ctx.backend.validate(ctx.diff));
    if (result.success) return;

    throw buildPlainTextError(result);
  },
});

function buildPlainTextError(res: ADCSDK.BackendValidateResult) {
  const lines: string[] = [];
  if (res.errorMessage) {
    lines.push(res.errorMessage);
  }
  for (const e of res.errors) {
    const parts: string[] = [e.resource_type];
    if (e.resource_name) {
      parts.push(`name="${e.resource_name}"`);
    } else {
      if (e.resource_id) parts.push(`id="${e.resource_id}"`);
      if (e.index !== undefined) parts.push(`index=${e.index}`);
    }
    lines.push(`  - [${parts.join(', ')}]: ${e.error}`);
  }
  const error = new Error(
    `Configuration validation failed:\n${lines.join('\n')}`,
  );
  error.stack = '';
  return error;
}
