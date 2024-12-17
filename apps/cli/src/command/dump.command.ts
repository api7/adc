import { BackendAPI7 } from '@api7/adc-backend-api7';
import * as ADCSDK from '@api7/adc-sdk';
import { Listr, ListrTask } from 'listr2';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { Scalar, stringify } from 'yaml';

import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BackendCommand } from './helper';
import type { BackendOptions } from './typing';
import {
  filterConfiguration,
  filterResourceType,
  loadBackend,
  recursiveRemoveMetadataField,
} from './utils';

type DumpOptions = BackendOptions & {
  output: string;
  withId: boolean;
};

export interface LoadRemoteConfigurationTaskOptions {
  backend: ADCSDK.Backend;
  labelSelector?: BackendOptions['labelSelector'];
  includeResourceType?: Array<ADCSDK.ResourceType>;
  excludeResourceType?: Array<ADCSDK.ResourceType>;
}
export const LoadRemoteConfigurationTask = ({
  backend,
  labelSelector,
  includeResourceType,
  excludeResourceType,
}: LoadRemoteConfigurationTaskOptions): ListrTask => ({
  title: 'Load remote configuration',
  task: async (ctx, task) => {
    return task.newListr([
      {
        title: 'Fetch all configuration',
        task: async () => await backend.dump(),
      },
      {
        title: 'Filter configuration resource type',
        enabled: () =>
          //TODO implement API-level resource filtering on APISIX backend
          !(backend instanceof BackendAPI7) &&
          (includeResourceType?.length > 0 || excludeResourceType?.length > 0),
        task: () => {
          ctx.remote = filterResourceType(
            ctx.remote,
            includeResourceType,
            excludeResourceType,
          );
        },
      },
      {
        title: 'Filter remote configuration',
        enabled: !!labelSelector,
        task: (ctx) => {
          [ctx.remote] = filterConfiguration(ctx.remote, labelSelector);
        },
      },
    ]);
  },
});

export const DumpCommand = new BackendCommand<DumpOptions>(
  'dump',
  'save the configuration of the backend to a file',
  'Save the configuration of the backend to the specified YAML file.',
)
  .option(
    '-o, --output <file-path>',
    'path of the file to save the configuration',
    'adc.yaml',
  )
  .option('--with-id', 'dump remote resources id')
  .addExamples([
    {
      title: 'Save backend configuration to the default adc.yaml file',
      command: 'adc dump',
    },
    {
      title: 'Save backend configuration to the specified file',
      command: 'adc dump -o service-configuration.yaml',
    },
    {
      title: 'Save only specified resource types from the backend',
      command:
        'adc dump --include-resource-type global_rule --include-resource-type plugin_metadata',
    },
    {
      title: 'Save only the resources with the specified labels',
      command: 'adc dump --label-selector app=catalog',
    },
    {
      title: 'Save the remote resources id',
      command: 'adc dump --with-id',
    },
  ])
  .handle(async (opts) => {
    const backend = loadBackend(opts.backend, opts);
    const tasks = new Listr<TaskContext, typeof SignaleRenderer>(
      [
        LoadRemoteConfigurationTask({
          backend,
          labelSelector: opts.labelSelector,
          includeResourceType: opts.includeResourceType,
          excludeResourceType: opts.excludeResourceType,
        }),
        {
          // Remove output resource metadata fields
          enabled: !opts.withId,
          task: (ctx) => recursiveRemoveMetadataField(ctx.remote),
        },
        {
          title: 'Write to dump file',
          task: async (ctx, task) => {
            await writeFile(
              opts.output,
              stringify(ctx.remote, {
                sortMapEntries: (a, b) => {
                  const nameKey = 'name';
                  const descKey = 'description';
                  const labelKey = 'labels';
                  const aKey = (a.key as Scalar)?.value;
                  const bKey = (b.key as Scalar)?.value;

                  // make sure the metadata is always at the top
                  if (aKey && bKey) {
                    if (aKey === nameKey || bKey === nameKey)
                      return aKey === nameKey ? -1 : 1;
                    if (aKey === descKey || bKey === descKey)
                      return aKey === descKey ? -1 : 1;
                    if (aKey === labelKey || bKey === labelKey)
                      return aKey === labelKey ? -1 : 1;
                  }

                  return a.key > b.key ? 1 : a.key < b.key ? -1 : 0;
                },
              }),
              {},
            );
            task.output = `Dump backend configuration to ${path.resolve(
              opts.output,
            )} successfully!`;
          },
        },
      ],
      {
        renderer: SignaleRenderer,
        rendererOptions: { verbose: opts.verbose },
        ctx: { remote: {}, local: {}, diff: [], defaultValue: {} },
      },
    );

    try {
      await tasks.run();
    } catch (err) {
      if (opts.verbose === 2) console.log(err);
      process.exit(1);
    }
  });
