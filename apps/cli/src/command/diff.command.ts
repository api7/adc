import * as ADCSDK from '@api7/adc-sdk';
import { Listr, ListrTask } from 'listr2';
import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import YAML from 'yaml';

import { DifferV3 } from '../differ/differv3';
import { SignaleRenderer } from '../utils/listr';
import { LoadRemoteConfigurationTask } from './dump.command';
import { BackendCommand, NoLintOption } from './helper';
import { LintTask } from './lint.command';
import { BackendOptions } from './typing';
import {
  fillLabels,
  filterResourceType,
  loadBackend,
  mergeKVConfigurations,
  recursiveReplaceEnvVars,
  toConfiguration,
  toKVConfiguration,
} from './utils';

type DiffOptions = BackendOptions & {
  file: Array<string>;
  lint: boolean;
};

export interface TaskContext {
  local?: ADCSDK.Configuration;
  remote?: ADCSDK.Configuration;
  diff?: ADCSDK.Event[];
  defaultValue?: ADCSDK.DefaultValue;
}

export const LoadLocalConfigurationTask = (
  files: Array<string>,
  labelSelector?: BackendOptions['labelSelector'],
  includeResourceType?: Array<ADCSDK.ResourceType>,
  excludeResourceType?: Array<ADCSDK.ResourceType>,
): ListrTask<{
  local: ADCSDK.Configuration;
}> => ({
  title: `Load local configuration`,
  task: async (ctx, task) => {
    if (!files || files.length <= 0) {
      task.output =
        'No configuration file input\nPlease specify the declarative configuration file to use with -f or --file';
      throw new Error();
    }

    interface LoadLocalContext {
      configurations: Record<string, ADCSDK.Configuration>;
    }
    const subCtx: LoadLocalContext = {
      configurations: {},
    };
    return task.newListr<LoadLocalContext>(
      [
        // load yaml files
        ...files.map((filePath): ListrTask<LoadLocalContext> => {
          return {
            title: `Load ${path.resolve(filePath)}`,
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

export const DiffResourceTask = (
  printSummary = false,
  persistentSummary = false,
): ListrTask<TaskContext> => ({
  title: 'Diff configuration',
  task: async (ctx, task) => {
    ctx.diff = DifferV3.diff(ctx.local, ctx.remote, ctx.defaultValue);

    if (printSummary) {
      task.output = '';
      let [created, updated, deleted] = [0, 0, 0];
      ctx.diff.forEach((event) => {
        switch (event.type) {
          case ADCSDK.EventType.CREATE:
            task.output += `create ${event.resourceType}: "${event.resourceName}"\n`;
            created++;
            break;
          case ADCSDK.EventType.DELETE:
            task.output += `delete ${event.resourceType}: "${event.resourceName}"\n`;
            deleted++;
            break;
          case ADCSDK.EventType.UPDATE:
            task.output += `update ${event.resourceType}: "${event.resourceName}"\n`;
            updated++;
            break;
        }
      });
      task.output += `Summary: ${created} will be created, ${updated} will be updated, ${deleted} will be deleted`;
    }
  },
});

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
      title: 'Compare configuration in specified file with the backend configuration',
      command: 'adc diff -f adc.yaml',
    },
    {
      title: 'Compare configuration in multiple specified files with the backend configuration',
      command: 'adc diff -f service-a.yaml -f service-b.yaml',
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
        LoadRemoteConfigurationTask({
          backend,
          labelSelector: opts.labelSelector,
          includeResourceType: opts.includeResourceType,
          excludeResourceType: opts.excludeResourceType,
        }),
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
      //console.log(chalk.red(`Failed to diff resources, ${err}`));
      process.exit(1);
    }
  });
