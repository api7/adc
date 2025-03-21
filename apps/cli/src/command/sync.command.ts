import { Listr } from 'listr2';
import { lastValueFrom, toArray } from 'rxjs';

import {
  DiffResourceTask,
  ExperimentalRemoteStateFileTask,
  LintTask,
  LoadLocalConfigurationTask,
  LoadRemoteConfigurationTask,
} from '../tasks';
import { InitializeBackendTask } from '../tasks/init_backend';
import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BackendCommand, NoLintOption } from './helper';
import { BackendOptions } from './typing';
import { addBackendEventListener } from './utils';

type SyncOption = BackendOptions & {
  file: Array<string>;
  lint: boolean;

  // experimental feature
  remoteStateFile: string;
};

export const SyncCommand = new BackendCommand<SyncOption>(
  'sync',
  'sync the local configuration to the backend',
  'Synchronize the configuration from the local file(s) to the backend.',
)
  .option(
    '-f, --file <file-path>',
    'file to synchronize',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
  .addOption(NoLintOption)
  .addExamples([
    {
      title: 'Synchronize configuration from a single file',
      command: 'adc sync -f adc.yaml',
    },
    {
      title: 'Synchronize configuration from multiple files',
      command: 'adc sync -f service-a.yaml -f service-b.yaml',
    },
    {
      title: 'Synchronize configuration in multiple files by glob expression',
      command: 'adc sync -f "**/*.yaml" -f common.yaml',
    },
    {
      title: 'Synchronize configuration to a specific gateway group',
      command: 'adc sync -f adc.yaml --gateway-group production',
    },
    {
      title: 'Synchronize configuration without lint check',
      command: 'adc sync -f adc.yaml --no-lint',
    },
    {
      title: 'Synchronize configuration with debug logs',
      command: 'adc sync -f adc.yaml --verbose 2',
    },
    {
      title:
        'Synchronize only specified resource types from the configuration file',
      command:
        'adc sync -f adc.yaml --include-resource-type global_rule --include-resource-type plugin_metadata',
    },
    {
      title: 'Synchronize only the resources with the specified lables',
      command: 'adc sync -f adc.yaml --label-selector app=catalog',
    },
  ])
  .handle(async (opts) => {
    const tasks = new Listr<TaskContext, typeof SignaleRenderer>(
      [
        InitializeBackendTask(opts.backend, opts),
        LoadLocalConfigurationTask(
          opts.file,
          opts.labelSelector,
          opts.includeResourceType,
          opts.excludeResourceType,
        ),
        opts.lint ? LintTask() : { task: () => undefined },
        !opts.remoteStateFile
          ? LoadRemoteConfigurationTask({
              labelSelector: opts.labelSelector,
              includeResourceType: opts.includeResourceType,
              excludeResourceType: opts.excludeResourceType,
            })
          : ExperimentalRemoteStateFileTask(opts.remoteStateFile),
        DiffResourceTask(false, false),
        {
          title: 'Sync configuration',
          task: async (ctx, task) => {
            const cancel = addBackendEventListener(ctx.backend, task);
            await lastValueFrom(
              ctx.backend
                .sync(ctx.diff, { exitOnFailure: true })
                .pipe(toArray()),
            );
            cancel();
          },
          exitOnError: true,
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
