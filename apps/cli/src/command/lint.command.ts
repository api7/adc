import { Listr } from 'listr2';

import { LintTask, LoadLocalConfigurationTask } from '../tasks';
import { SignaleRenderer } from '../utils/listr';
import { BaseCommand } from './helper';

export const LintCommand = new BaseCommand('lint')
  .description(
    'Lint the local configuration file(s) to ensure it meets ADC requirements.',
  )
  .summary('lint the local configuration')
  .option(
    '-f, --file <file-path>',
    'file to lint',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
  .addExamples([
    {
      title: 'Lint the specified configuration file',
      command: 'adc lint -f adc.yaml',
    },
    {
      title: 'Lint multiple configuration files',
      command: 'adc lint -f service-a.yaml -f service-b.yaml',
    },
  ])
  .action(async () => {
    const opts = LintCommand.optsWithGlobals();

    const tasks = new Listr(
      [LoadLocalConfigurationTask(opts.file, {}), LintTask()],
      {
        renderer: SignaleRenderer,
        rendererOptions: { verbose: opts.verbose },
      },
    );

    try {
      await tasks.run();
    } catch (err) {
      process.exit(1);
    }
  });
