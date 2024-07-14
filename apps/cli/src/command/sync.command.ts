import { Listr } from 'listr2';

import { SignaleRenderer } from '../utils/listr';
import {
  DiffResourceTask,
  LoadLocalConfigurationTask,
  TaskContext,
} from './diff.command';
import { LoadRemoteConfigurationTask } from './dump.command';
import { BackendCommand } from './helper';
import { BackendOptions } from './typing';
import { loadBackend } from './utils';

type SyncOption = BackendOptions & {
  file: Array<string>;
};

export const SyncCommand = new BackendCommand<SyncOption>(
  'sync',
  'Sync local configurations to backend',
)
  .option(
    '-f, --file <file-path>',
    'The files you want to synchronize, can be set more than one.',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
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
