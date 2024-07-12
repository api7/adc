import { BackendAPI7 } from '../src';
import { getDefaultValue } from './support/utils';

describe('Default Value', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: globalThis.server,
      token: globalThis.token,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  it('Check default value', async () => {
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
                    200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301,
                    302, 303, 304, 305, 306, 307, 308,
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
  });
});
