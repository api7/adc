import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';
import pluralize from 'pluralize';
import { ZodError } from 'zod';

import { check } from '../linter';

export const LintTask = (): ListrTask<{ local: ADCSDK.Configuration }> => ({
  title: 'Lint configuration',
  task: (ctx) => {
    const result = check(ctx.local);

    if (!result.success) {
      let err =
        'Lint configuration\nThe following errors were found in configuration:\n';
      let pathException = false;

      if ('error' in result) {
        (result.error as ZodError).errors.forEach((error, idx) => {
          if (error.path.length < 2) {
            // special case: not enough information is available to indicate the location of the error
            pathException = true;
            err += `#${idx + 1} raw error: ${JSON.stringify(error)}\n`;
            return;
          }

          // normal case
          const resourceType = pluralize.singular(error.path[0] as string);
          const resource = ctx.local[error.path[0]][error.path[1]];
          const resourceName =
            resourceType === 'global_rule' || resourceType === 'plugin_metadata'
              ? error.path[1]
              : resourceType === 'ssl'
                ? resource.snis
                : resourceType === 'consumer'
                  ? resource.username
                  : resource.name;
          const field = (error.path.slice(2, error.path.length) ?? []).join(
            '.',
          );
          err += `#${idx + 1} ${
            error.message
          } at ${resourceType}: "${resourceName}"${field ? `, field: "${field}"` : ''}\n`;
        });
      }

      if (pathException)
        err +=
          'NOTE: There are some unsummarizable errors in the lint results that are presented as "raw error". You can report such unexpected cases.';

      const error = new Error(err);
      error.stack = '';
      throw error;
    }

    ctx.local = result.data as ADCSDK.Configuration;
  },
});
