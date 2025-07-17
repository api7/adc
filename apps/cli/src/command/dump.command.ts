import { dump } from 'js-yaml';
import { Listr } from 'listr2';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

import { LoadRemoteConfigurationTask } from '../tasks';
import { InitializeBackendTask } from '../tasks/init_backend';
import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BackendCommand } from './helper';
import type { BackendOptions } from './typing';
import { recursiveRemoveMetadataField, resortConfiguration } from './utils';

type DumpOptions = BackendOptions & {
  output: string;
  withId: boolean;
};

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
    const tasks = new Listr<TaskContext, typeof SignaleRenderer>(
      [
        InitializeBackendTask(opts.backend, opts),
        LoadRemoteConfigurationTask({
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
              dump(resortConfiguration(ctx.remote), {
                noRefs: true,
                sortKeys: true,
              }),
              { encoding: 'utf8' },
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
        ctx: { remote: {}, local: {}, diff: [] },
      },
    );

    try {
      await tasks.run();
    } catch (err) {
      if (opts.verbose === 2) console.log(err);
      process.exit(1);
    }
  });
