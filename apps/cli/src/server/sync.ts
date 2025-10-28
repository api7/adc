import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import type { RequestHandler } from 'express';
import { omit, toString } from 'lodash';
import { lastValueFrom, toArray } from 'rxjs';

import {
  fillLabels,
  filterConfiguration,
  filterResourceType,
  loadBackend,
} from '../command/utils';
import { check } from '../linter';
import { logger } from './logger';
import { SyncInput, type SyncInputType } from './schema';

export const syncHandler: RequestHandler<
  unknown,
  unknown,
  SyncInputType
> = async (req, res) => {
  try {
    const parsedInput = SyncInput.safeParse(req.body);
    if (!parsedInput.success)
      return res.status(400).json({
        message: parsedInput.error.message,
        errors: parsedInput.error.issues,
      });
    const { task } = parsedInput.data;

    // load local configuration and validate it
    //TODO: merged with the listr task
    const local = filterResourceType(
      task.config,
      task.opts.includeResourceType,
      task.opts.excludeResourceType,
    ) as ADCSDK.Configuration;
    if (task.opts.lint) {
      const result = check(local);
      if (!result.success)
        return res.status(400).json({
          message: result.error.message,
          errors: result.error.issues,
        });
    }
    fillLabels(local, task.opts.labelSelector);

    // load and filter remote configuration
    //TODO: merged with the listr task
    const backend = loadBackend(task.opts.backend, {
      ...task.opts,
      server: Array.isArray(task.opts.server)
        ? task.opts.server.join(',')
        : task.opts.server,
    });

    backend.on('AXIOS_DEBUG', ({ description, response }) =>
      logger.log({
        level: 'debug',
        message: description,
        request: {
          method: response.config.method,
          url: response.config.url,
          headers: response.config.headers,
          data: response.config.data,
        },
        response: {
          status: response.status,
          headers: response.headers,
          data: response.data,
        },
        requestId: req.requestId,
      }),
    );

    let remote = await lastValueFrom(backend.dump());
    remote = filterResourceType(
      remote,
      task.opts.includeResourceType,
      task.opts.excludeResourceType,
    );
    [remote] = filterConfiguration(remote, task.opts.labelSelector);

    // diff local and remote configuration
    const diff = DifferV3.diff(
      local,
      remote,
      await backend.defaultValue(),
      undefined,
      {
        log: (message: string) =>
          logger.log({ level: 'debug', message, requestId: req.requestId }),
        debug: (logEntry) =>
          logger.log({ level: 'debug', ...logEntry, requestId: req.requestId }),
      },
    );

    // sync the diff
    const results = await lastValueFrom(
      backend
        .sync(diff, {
          exitOnFailure: false,
          concurrent: 4,
        })
        .pipe(toArray()),
    );
    const successes = results.filter((result) => result.success);
    const faileds = results.filter((result) => !result.success);
    const simplifyEvent = (event: ADCSDK.Event) =>
      omit(event, ['diff', 'oldValue', 'newValue', 'subEvents']);
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
        ...successes.map(({ event, axiosResponse, server }) => ({
          server,
          event: simplifyEvent(event),
          synced_at: new Date(
            axiosResponse?.headers?.date ?? new Date(),
          ).toISOString(),
        })),
      ],
      failed: [
        ...faileds.map(({ event, error, axiosResponse, server }) => ({
          server,
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

    logger.log({
      level: 'debug',
      message: 'sync finished',
      output,
      requestId: req.requestId,
    });
    res.status(202).json(output);
  } catch (err) {
    logger.log({
      level: 'debug',
      message: 'sync failed',
      error: err,
      requestId: req.requestId,
    });
    res.status(500).json({
      message: toString(err),
    });
  }
};
