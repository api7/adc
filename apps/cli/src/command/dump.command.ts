import { BackendAPI7 } from '@api7/adc-backend-api7';
import * as ADCSDK from '@api7/adc-sdk';
import { Listr, ListrTask } from 'listr2';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { stringify } from 'yaml';

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
  .addExamples([
    {
      title: 'Save backend configuration to the default adc.yaml file',
      command: 'adc dump'
    },
    {
      title: 'Save backend configuration to the specified file',
      command: 'adc dump -o service-configuration.yaml'
    },
    {
      title: 'Save only specified resource types from the backend',
      command: 'adc dump --include-resource-type global_rule --include-resource-type plugin_metadata',
    },
    {
      title: 'Save only the resources with the specified labels',
      command: 'adc dump --label-selector app=catalog',
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
          task: (ctx) => recursiveRemoveMetadataField(ctx.remote),
        },
        {
          title: 'Write to dump file',
          task: async (ctx, task) => {
            await writeFile(opts.output, stringify(ctx.remote), {});
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
      //console.log(chalk.red(`Failed to dump backend configuration from backend, ${err}`));
      process.exit(1);
    }
  });
