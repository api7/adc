import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { HttpAgent, HttpOptions, HttpsAgent } from 'agentkeepalive';
import type { RequestHandler } from 'express';
import { toString } from 'lodash-es';
import { lastValueFrom } from 'rxjs';

import { fillLabels, filterResourceType, loadBackend } from '../command/utils';
import { check } from '../linter';
import { logger } from './logger';
import { ValidateInput, type ValidateInputType } from './schema';

// create connection pool
const keepAlive: HttpOptions = {
  keepAlive: true,
  maxSockets: 256, // per host
  maxFreeSockets: 16, // per host free
  freeSocketTimeout:
    parseInt(process.env.ADC_INGRESS_FREE_SOCKET_TIMEOUT) || 50000,
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

export const validateHandler: RequestHandler<
  unknown,
  unknown,
  ValidateInputType
> = async (req, res) => {
  try {
    const parsedInput = ValidateInput.safeParse(req.body);
    if (!parsedInput.success)
      return res.status(400).json({
        success: false,
        message: parsedInput.error.message,
        errors: parsedInput.error.issues,
      });
    const { task } = parsedInput.data;

    // load local configuration and filter resource types
    const local = filterResourceType(
      task.config,
      task.opts.includeResourceType,
      task.opts.excludeResourceType,
    ) as ADCSDK.InternalConfiguration;

    // optional lint
    if (task.opts.lint) {
      const result = check(local);
      if (!result.success)
        return res.status(400).json({
          success: false,
          message: result.error.message,
          errors: result.error.issues,
        });
    }
    fillLabels(local, task.opts.labelSelector);

    // initialize backend
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

    // generate events by diffing against an empty remote config
    const events = DifferV3.diff(
      local,
      {} as ADCSDK.InternalConfiguration,
      await backend.defaultValue(),
      undefined,
      {
        log: (message: string) =>
          logger.log({ level: 'debug', message, requestId: req.requestId }),
        debug: (logEntry) =>
          logger.log({ level: 'debug', ...logEntry, requestId: req.requestId }),
      },
    );

    // check if backend supports validate
    if (!backend.validate)
      return res.status(400).json({
        success: false,
        message: 'Validate is not supported by the current backend.',
        errors: [],
      });

    // execute validation
    const result = await lastValueFrom(backend.validate(events));

    logger.log({
      level: 'debug',
      message: 'validate finished',
      success: result.success,
      errors: result.errors,
      requestId: req.requestId,
    });

    res.status(200).json({
      success: result.success,
      ...(result.errorMessage ? { message: result.errorMessage } : {}),
      errors: result.errors,
    });
  } catch (err) {
    logger.log({
      level: 'debug',
      message: 'validate failed',
      error: err,
      requestId: req.requestId,
    });
    res.status(500).json({
      success: false,
      message: toString(err),
      errors: [],
    });
  }
};
