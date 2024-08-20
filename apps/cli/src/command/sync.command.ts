import { Listr } from 'listr2';

import { SignaleRenderer } from '../utils/listr';
import {
  DiffResourceTask,
  LoadLocalConfigurationTask,
  TaskContext,
} from './diff.command';
import { LoadRemoteConfigurationTask } from './dump.command';
import { BackendCommand, NoLintOption } from './helper';
import { LintTask } from './lint.command';
import { BackendOptions } from './typing';
import { loadBackend } from './utils';

type SyncOption = BackendOptions & {
  file: Array<string>;
  lint: boolean;
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
  .addExample('adc sync -f adc.yaml')
  .addExample('adc sync -f service-a.yaml -f service-b.yaml')
  .handle(async (opts) => {
    const backend = loadBackend(opts.backend, opts);

    const tasks = new Listr<TaskContext, typeof SignaleRenderer>(
      [
        LoadLocalConfigurationTask(
          opts.file,
          opts.labelSelector,
          opts.includeResourceType,
          opts.excludeResourceType,
        ),
        opts.lint ? LintTask() : { task: () => undefined },
        LoadRemoteConfigurationTask({
          backend,
          labelSelector: opts.labelSelector,
          includeResourceType: opts.includeResourceType,
          excludeResourceType: opts.excludeResourceType,
        }),
        DiffResourceTask(false, false),
        {
          title: 'Sync configuration',
          task: async () => await backend.sync(),
          exitOnError: true,
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
      process.exit(1);
    }
  });
