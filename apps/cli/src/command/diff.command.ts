import * as ADCSDK from '@api7/adc-sdk';
import { Listr } from 'listr2';
import { readFile, writeFile } from 'node:fs/promises';
import YAML from 'yaml';

import {
  DiffResourceTask,
  ExperimentalRemoteStateFileTask,
  LintTask,
  LoadLocalConfigurationTask,
} from '../tasks';
import { LoadRemoteConfigurationTask } from '../tasks';
import { SignaleRenderer } from '../utils/listr';
import { BackendCommand, NoLintOption } from './helper';
import { BackendOptions } from './typing';
import { loadBackend } from './utils';

type DiffOptions = BackendOptions & {
  file: Array<string>;
  lint: boolean;

  // experimental feature
  remoteStateFile: string;
};

export interface TaskContext {
  local?: ADCSDK.Configuration;
  remote?: ADCSDK.Configuration;
  diff?: ADCSDK.Event[];
  defaultValue?: ADCSDK.DefaultValue;
}

export const DiffCommand = new BackendCommand<DiffOptions>(
  'diff',
  'show differences between the local and the backend configurations',
  'Compare the configuration in the specified file(s) with the backend configuration',
)
  .option(
    '-f, --file <file-path>',
    'file to compare',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
  .addOption(NoLintOption)
  .addExamples([
    {
      title:
        'Compare configuration in a specified file with the backend configuration',
      command: 'adc diff -f adc.yaml',
    },
    {
      title:
        'Compare configuration in multiple specified files with the backend configuration',
      command: 'adc diff -f service-a.yaml -f service-b.yaml',
    },
    {
      title:
        'Compare configuration in multiple specified files by glob expression',
      command: 'adc diff -f "**/*.yaml" -f common.yaml',
    },
  ])
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
        !opts.remoteStateFile
          ? LoadRemoteConfigurationTask({
              backend,
              labelSelector: opts.labelSelector,
              includeResourceType: opts.includeResourceType,
              excludeResourceType: opts.excludeResourceType,
            })
          : ExperimentalRemoteStateFileTask(opts.remoteStateFile),
        DiffResourceTask(true, true),
        {
          title: 'Write detail diff result to file',
          task: async (ctx, task) => {
            await writeFile('./diff.yaml', YAML.stringify(ctx.diff), {});
            task.output = 'Detail diff result has been wrote to diff.yaml';
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
      process.exit(1);
    }
  });
