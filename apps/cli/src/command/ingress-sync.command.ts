import * as ADCSDK from '@api7/adc-sdk';
import { Listr, SilentRenderer } from 'listr2';
import { omit } from 'lodash';
import { lastValueFrom, toArray } from 'rxjs';

import {
  DiffResourceTask,
  ExperimentalRemoteStateFileTask,
  LintTask,
  LoadLocalConfigurationTask,
  LoadRemoteConfigurationTask,
} from '../tasks';
import { InitializeBackendTask } from '../tasks/init_backend';
import { TaskContext } from './diff.command';
import { BackendCommand } from './helper';
import { BackendOptions } from './typing';

type SyncOption = BackendOptions & {
  file: Array<string>;
  lint: boolean;

  // experimental feature
  remoteStateFile: string;
};

export const IngressSyncCommand = new BackendCommand<SyncOption>('sync')
  .option(
    '-f, --file <file-path>',
    'file to synchronize',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
  .handle(async (opts) => {
    const tasks = new Listr<TaskContext, typeof SilentRenderer>(
      [
        InitializeBackendTask(opts.backend, opts),
        LoadLocalConfigurationTask(
          opts.file,
          opts.labelSelector,
          opts.includeResourceType,
          opts.excludeResourceType,
        ),
        LintTask(),
        !opts.remoteStateFile
          ? LoadRemoteConfigurationTask({
              labelSelector: opts.labelSelector,
              includeResourceType: opts.includeResourceType,
              excludeResourceType: opts.excludeResourceType,
            })
          : ExperimentalRemoteStateFileTask(opts.remoteStateFile),
        {
          task: (ctx) => {
            ctx.remote = {};
          },
        },
        DiffResourceTask(false, false),
        {
          title: 'Sync configuration',
          task: async (ctx) => {
            try {
              const results = await lastValueFrom(
                ctx.backend.sync(ctx.diff).pipe(toArray()),
              );

              const successes = results.filter((result) => result.success);
              const faileds = results.filter((result) => !result.success);
              const simplifyEvent = (event: ADCSDK.Event) => {
                return omit(event, [
                  'diff',
                  'oldValue',
                  'newValue',
                  'subEvents',
                ]);
              };
              const output = {
                status:
                  results.length === successes.length
                    ? 'success'
                    : results.length === faileds.length
                      ? 'all_failed'
                      : 'partial_failure',
                total_resources: results.length,
                success_count: successes.length,
                failed_count: faileds.length,
                success: [
                  ...successes.map(({ event, axiosResponse }) => ({
                    event: simplifyEvent(event),
                    synced_at: new Date(
                      axiosResponse?.headers?.date ?? new Date(),
                    ).toISOString(),
                  })),
                ],
                failed: [
                  ...faileds.map(({ event, error, axiosResponse }) => ({
                    event: simplifyEvent(event),
                    failed_at: new Date(
                      axiosResponse?.headers?.date ?? new Date(),
                    ).toISOString(),
                    reason: error.message,
                    ...(axiosResponse && {
                      response: {
                        status: axiosResponse.status,
                        headers: axiosResponse.headers,
                        data: axiosResponse.data,
                      },
                    }),
                  })),
                ],
              };
              process.stdout.write(JSON.stringify(output));
            } catch (err) {
              process.stderr.write(err);
            }
          },
        },
      ],
      {
        renderer: SilentRenderer,
        ctx: { remote: {}, local: {}, diff: [] },
      },
    );

    try {
      await tasks.run();
    } catch (err) {
      process.stderr.write(err);
      process.exit(1);
    }
  });
