import { Listr } from 'listr2';

import { LintTask, LoadLocalConfigurationTask, ValidateTask } from '../tasks';
import { InitializeBackendTask } from '../tasks/init_backend';
import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BackendCommand, NoLintOption } from './helper';
import { BackendOptions } from './typing';

export type ValidateOptions = BackendOptions & {
  file: Array<string>;
  lint: boolean;
};

export const ValidateCommand = new BackendCommand<ValidateOptions>(
  'validate',
  'validate the local configuration against the backend',
  'Validate the configuration from the local file(s) against the backend without applying any changes.',
)
  .option(
    '-f, --file <file-path>',
    'file to validate',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
  .addOption(NoLintOption)
  .addExamples([
    {
      title: 'Validate configuration from a single file',
      command: 'adc validate -f adc.yaml',
    },
    {
      title: 'Validate configuration from multiple files',
      command: 'adc validate -f service-a.yaml -f service-b.yaml',
    },
    {
      title: 'Validate configuration against API7 EE backend',
      command:
        'adc validate -f adc.yaml --backend api7ee --gateway-group default',
    },
    {
      title: 'Validate configuration without lint check',
      command: 'adc validate -f adc.yaml --no-lint',
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
        ValidateTask(),
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
