import { globalAgent as httpAgent } from 'node:http';
import { gte, lt } from 'semver';

import { BackendAPI7 } from '../src';
import {
  conditionalIt,
  generateHTTPSAgent,
  getDefaultValue,
  semverCondition,
} from './support/utils';

describe('Default Value', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER!,
      token: process.env.TOKEN!,
      tlsSkipVerify: true,
      gatewayGroup: process.env.GATEWAY_GROUP,
      cacheKey: 'default',
      httpAgent,
      httpsAgent: generateHTTPSAgent(),
    });
  });

  conditionalIt(semverCondition(lt, '3.6.0'))(
    'Check default value (<3.6.0)',
    async () => {
      const defaultValue = await getDefaultValue(backend);
      expect(defaultValue).toMatchObject({
        core: {
          service: {
            upstream: {
              checks: {
                active: {
                  concurrency: 10,
                  healthy: {
                    http_statuses: [200, 302],
                    interval: 1,
                    successes: 2,
                  },
                  http_path: '/',
                  https_verify_certificate: true,
                  timeout: 1,
                  type: 'http',
                  unhealthy: {
                    http_failures: 5,
                    http_statuses: [429, 404, 500, 501, 502, 503, 504, 505],
                    interval: 1,
                    tcp_failures: 2,
                    timeouts: 3,
                  },
                },
                passive: {
                  healthy: {
                    http_statuses: [
                      200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300,
                      301, 302, 303, 304, 305, 306, 307, 308,
                    ],
                    successes: 5,
                  },
                  type: 'http',
                  unhealthy: {
                    http_failures: 5,
                    http_statuses: [429, 500, 503],
                    tcp_failures: 2,
                    timeouts: 7,
                  },
                },
              },
              discovery_args: {},
              hash_on: 'vars',
              keepalive_pool: { idle_timeout: 60, requests: 1000, size: 320 },
              name: 'default',
              nodes: [{ priority: 0 }],
              pass_host: 'pass',
              retry_timeout: 0,
              scheme: 'http',
              timeout: { connect: 60, read: 60, send: 60 },
              type: 'roundrobin',
            },
          },
          ssl: { client: { depth: 1 } },
        },
      });
    },
  );

  conditionalIt(semverCondition(gte, '3.6.0') && semverCondition(lt, '3.8.0'))(
    'Check default value (>=3.6.0 && <3.8.0)',
    async () => {
      const defaultValue = await getDefaultValue(backend);
      expect(defaultValue).toMatchObject({
        core: {
          service: {
            strip_path_prefix: true,
          },
          ssl: { client: { depth: 1 } },
        },
      });
    },
  );

  conditionalIt(semverCondition(gte, '3.8.0'))(
    'Check default value (>=3.8.0)',
    async () => {
      const defaultValue = await getDefaultValue(backend);
      expect(defaultValue).toMatchObject({
        core: {
          consumer: {},
          consumer_credential: {},
          global_rule: {},
          plugin_metadata: {},
          route: {
            priority: 0,
            timeout: {
              connect: 60,
              read: 60,
              send: 60,
            },
          },
          service: {
            strip_path_prefix: true,
            upstream: {
              discovery_args: {},
              hash_on: 'vars',
              keepalive_pool: {
                idle_timeout: 60,
                requests: 1000,
                size: 320,
              },
              name: 'default',
              nodes: [
                {
                  priority: 0,
                },
              ],
              pass_host: 'pass',
              retry_timeout: 0,
              scheme: 'http',
              timeout: {
                connect: 60,
                read: 60,
                send: 60,
              },
              type: 'roundrobin',
            },
          },
          ssl: {
            certificates: [],
            snis: [],
          },
          stream_route: {},
          stream_service: {
            upstream: {
              hash_on: 'vars',
              name: 'default',
              nodes: [
                {
                  priority: 0,
                },
              ],
              retry_timeout: 0,
              scheme: 'tcp',
              timeout: {
                connect: 60,
                read: 60,
                send: 60,
              },
              type: 'roundrobin',
            },
          },
          upstream: {
            discovery_args: {},
            hash_on: 'vars',
            keepalive_pool: {
              idle_timeout: 60,
              requests: 1000,
              size: 320,
            },
            name: 'default',
            nodes: [
              {
                priority: 0,
              },
            ],
            pass_host: 'pass',
            retry_timeout: 0,
            scheme: 'http',
            timeout: {
              connect: 60,
              read: 60,
              send: 60,
            },
            type: 'roundrobin',
          },
        },
      });
    },
  );
});
