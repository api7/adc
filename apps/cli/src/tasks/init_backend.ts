import * as ADCSDK from '@api7/adc-sdk';
import { HttpAgent, HttpOptions, HttpsAgent } from 'agentkeepalive';
import { ListrTask } from 'listr2';
import { readFileSync } from 'node:fs';

import { loadBackend } from '../command/utils';

export const InitializeBackendTask = (
  type: string,
  opts: ADCSDK.BackendOptions,
): ListrTask => ({
  task: async (ctx) => {
    const keepAlive: HttpOptions = {
      keepAlive: true,
      maxSockets: 256, // per host
      maxFreeSockets: 16, // per host free
      freeSocketTimeout: 60000,
    };

    ctx.backend = loadBackend(type, {
      ...opts,
      cacheKey: 'default',
      httpAgent: new HttpAgent(keepAlive),
      httpsAgent: new HttpsAgent({
        rejectUnauthorized: !opts?.tlsSkipVerify,
        ...keepAlive,
        ...(opts?.caCertFile ? { ca: readFileSync(opts.caCertFile) } : {}),
        ...(opts?.tlsClientCertFile
          ? { cert: readFileSync(opts.tlsClientCertFile) }
          : {}),
        ...(opts?.tlsClientKeyFile
          ? { key: readFileSync(opts.tlsClientKeyFile) }
          : {}),
      }),
    });
  },
});
