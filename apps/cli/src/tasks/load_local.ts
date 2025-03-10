import * as ADCSDK from '@api7/adc-sdk';
import { existsSync } from 'fs';
import { readFile } from 'fs/promises';
import { globSync } from 'glob';
import { ListrTask } from 'listr2';
import path from 'node:path';
import YAML from 'yaml';

import {
  fillLabels,
  filterResourceType,
  mergeKVConfigurations,
  recursiveReplaceEnvVars,
  toConfiguration,
  toKVConfiguration,
} from '../command/utils';

export const LoadLocalConfigurationTask = (
  files: Array<string>,
  labelSelector?: ADCSDK.BackendOptions['labelSelector'],
  includeResourceType?: Array<ADCSDK.ResourceType>,
  excludeResourceType?: Array<ADCSDK.ResourceType>,
): ListrTask<{
  local: ADCSDK.Configuration;
}> => ({
  title: `Load local configuration`,
  task: async (ctx, task) => {
    if (!files || files.length <= 0) files = ['adc.yaml'];

    interface LoadLocalContext {
      configurations: Record<string, ADCSDK.Configuration>;
    }
    const subCtx: LoadLocalContext = {
      configurations: {},
    };
    return task.newListr<LoadLocalContext>(
      [
        // load yaml files
        ...files
          .flatMap((filePath) =>
            globSync(filePath, { nodir: true, absolute: true }),
          )
          .map((filePath): ListrTask<LoadLocalContext> => {
            return {
              title: `Load ${filePath}`,
              task: async (subCtx) => {
                if (!existsSync(filePath))
                  throw new Error(
                    `File ${path.resolve(filePath)} does not exist`,
                  );
                const fileContent =
                  (await readFile(filePath, { encoding: 'utf-8' })) ?? '';

                subCtx.configurations[filePath] = YAML.parse(fileContent) ?? {};
              },
            };
          }),
        // merge yaml files
        {
          title: 'Merge local configurations',
          task: async (subCtx) => {
            const localKVConfiguration = mergeKVConfigurations(
              Object.fromEntries(
                Object.entries(subCtx.configurations).map(
                  ([filePath, configuration]) => [
                    filePath,
                    toKVConfiguration(configuration, filePath),
                  ],
                ),
              ),
            );
            ctx.local = toConfiguration(localKVConfiguration);
          },
        },
        {
          title: 'Resolve value variables',
          task: async () => {
            ctx.local = recursiveReplaceEnvVars(ctx.local);
          },
        },
        {
          title: 'Filter configuration resource type',
          enabled: () =>
            includeResourceType?.length > 0 || excludeResourceType?.length > 0,
          task: () => {
            ctx.local = filterResourceType(
              ctx.local,
              includeResourceType,
              excludeResourceType,
            );
          },
        },
        {
          title: 'Filter configuration',
          enabled: !!labelSelector && Object.keys(labelSelector).length > 0,
          task: async () => {
            // Merge label selectors from CLI inputs to each resource
            fillLabels(ctx.local, labelSelector);
          },
        },
      ],
      {
        ctx: subCtx,
      },
    );
  },
});
