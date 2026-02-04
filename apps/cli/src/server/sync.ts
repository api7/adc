import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { HttpAgent, HttpOptions, HttpsAgent } from 'agentkeepalive';
import { AxiosResponse } from 'axios';
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

// create connection pool
const keepAlive: HttpOptions = {
  keepAlive: true,
  maxSockets: 256, // per host
  maxFreeSockets: 16, // per host free
  freeSocketTimeout:
    parseInt(process.env.ADC_INGRESS_FREE_SOCKET_TIMEOUT) ?? 50000, // free socket keepalive for 50 seconds, and if the ADC_INGRESS_FREE_SOCKET_TIMEOUT environment variable is provided, it takes precedence.
};
const httpAgent = new HttpAgent(keepAlive);

//TODO: dynamic rejectUnauthorized and support mTLS
const httpsAgent = new HttpsAgent({
  rejectUnauthorized: true,
  ...keepAlive,
});
const httpsInsecureAgent = new HttpsAgent({
  rejectUnauthorized: false,
  ...keepAlive,
});

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
    ) as ADCSDK.InternalConfiguration;
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
    const backend = loadBackend(task.opts.backend, {
      ...task.opts,
      server: Array.isArray(task.opts.server)
        ? task.opts.server.join(',')
        : task.opts.server,
      httpAgent,
      httpsAgent: task.opts.tlsSkipVerify ? httpsInsecureAgent : httpsAgent,
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
      remote as ADCSDK.InternalConfiguration,
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

    const output =
      task.opts.backend !== 'apisix-standalone'
        ? generateOutput([results, successes, faileds])
        : generateOutputForAPISIXStandalone(diff, [
            results,
            successes,
            faileds,
          ]);

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

const simplifyEvent = (event: ADCSDK.Event) =>
  omit(event, ['diff', 'oldValue', 'newValue', 'subEvents']);

const formatAxiosResponse = (axiosResponse: AxiosResponse) => ({
  response: {
    status: axiosResponse.status,
    headers: axiosResponse.headers,
    data: axiosResponse.data,
  },
  request: {
    url: axiosResponse.config.url,
    method: axiosResponse.config.method,
    headers:
      ((axiosResponse.config.headers['X-API-KEY'] = '*****'),
      axiosResponse.config.headers),
    data: axiosResponse.config.data,
  },
});

const generateOutput = ([results, successes, faileds]: [
  Array<ADCSDK.BackendSyncResult>,
  Array<ADCSDK.BackendSyncResult>,
  Array<ADCSDK.BackendSyncResult>,
]) => {
  const date = new Date();
  return {
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
        synced_at: new Date(axiosResponse?.headers?.date ?? date).toISOString(),
      })),
    ],
    failed: [
      ...faileds.map(({ event, error, axiosResponse, server }) => ({
        server,
        event: simplifyEvent(event),
        failed_at: new Date(axiosResponse?.headers?.date ?? date).toISOString(),
        reason: error.message,
        ...(axiosResponse && formatAxiosResponse(axiosResponse)),
      })),
    ],
  };
};

/**
 * The `sync` of the APISIX Standalone backend returns whether sync on the endpoint succeeded or failed,
 * while other backends should return the synchronization success status for each individual Event.
 * This represents a significant difference in backend behavior, as the APISIX Standalone backend
 * always initiates only one request regardless of how many Events require synchronization.
 *
 * According to the standard of the ADC server sync API, we will use the `success` array to store
 * all events and add an additional `endpoint_status` to track the success of synchronization
 * across each backend.
 * @param events
 * @param results
 * @returns
 */
const generateOutputForAPISIXStandalone = (
  events: Array<ADCSDK.Event>,
  [results, successes, faileds]: [
    Array<ADCSDK.BackendSyncResult>,
    Array<ADCSDK.BackendSyncResult>,
    Array<ADCSDK.BackendSyncResult>,
  ],
) => {
  const date = new Date();
  return {
    status:
      results.length === successes.length
        ? 'success'
        : results.length === faileds.length
          ? 'all_failed'
          : 'partial_failure',
    total_resources: 0,
    success_count: successes.length,
    failed_count: faileds.length,
    success: [
      ...events.map((event) => ({
        event: simplifyEvent(event),
        synced_at: date.toISOString(),
      })),
    ],
    failed: [],
    endpoint_status: results.map((result) => {
      return {
        server: result.server,
        success: result.success,
        reason: result?.error?.message,
        requested_at: new Date(
          result.axiosResponse?.headers?.date ?? date,
        ).toISOString(),
        ...(result.axiosResponse && formatAxiosResponse(result.axiosResponse)),
      };
    }),
  };
};
