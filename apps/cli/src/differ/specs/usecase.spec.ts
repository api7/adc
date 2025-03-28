import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3';

describe('Differ V3 - usecase', () => {
  it('rename service with nested routes', () => {
    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: 'HTTPBIN Service1',
              routes: [
                {
                  name: 'Anything',
                  methods: ['GET'],
                  uris: ['/anything'],
                },
                {
                  name: 'Generate UUID',
                  methods: ['GET'],
                  uris: ['/uuid'],
                },
              ],
              upstream: {
                scheme: 'http',
                nodes: [
                  {
                    host: 'httpbin.org',
                    port: 80,
                    weight: 1,
                    priority: 0,
                  },
                ],
              },
            },
          ],
        },
        {
          services: [
            {
              id: ADCSDK.utils.generateId('HTTPBIN Service'),
              name: 'HTTPBIN Service',
              description: '',
              routes: [
                {
                  id: ADCSDK.utils.generateId('HTTPBIN Service.Anything'),
                  name: 'Anything',
                  methods: ['GET'],
                  uris: ['/anything'],
                },
                {
                  id: ADCSDK.utils.generateId('HTTPBIN Service.Generate UUID'),
                  name: 'Generate UUID',
                  methods: ['GET'],
                  uris: ['/uuid'],
                },
              ],
              upstream: {
                scheme: 'http',
                nodes: [
                  {
                    host: 'httpbin.org',
                    port: 80,
                    weight: 1,
                    priority: 0,
                  },
                ],
              },
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: { methods: ['GET'], name: 'Anything', uris: ['/anything'] },
        parentId: ADCSDK.utils.generateId('HTTPBIN Service'),
        resourceId: ADCSDK.utils.generateId('HTTPBIN Service.Anything'),
        resourceName: 'Anything',
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: { methods: ['GET'], name: 'Generate UUID', uris: ['/uuid'] },
        parentId: ADCSDK.utils.generateId('HTTPBIN Service'),
        resourceId: ADCSDK.utils.generateId('HTTPBIN Service.Generate UUID'),
        resourceName: 'Generate UUID',
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: {
          description: '',
          name: 'HTTPBIN Service',
          routes: [
            {
              id: ADCSDK.utils.generateId('HTTPBIN Service.Anything'),
              methods: ['GET'],
              name: 'Anything',
              uris: ['/anything'],
            },
            {
              id: ADCSDK.utils.generateId('HTTPBIN Service.Generate UUID'),
              name: 'Generate UUID',
              methods: ['GET'],
              uris: ['/uuid'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 80, priority: 0, weight: 1 }],
            scheme: 'http',
          },
        },
        resourceId: ADCSDK.utils.generateId('HTTPBIN Service'),
        resourceName: 'HTTPBIN Service',
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        newValue: {
          name: 'HTTPBIN Service1',
          routes: [
            { methods: ['GET'], name: 'Anything', uris: ['/anything'] },
            {
              name: 'Generate UUID',
              methods: ['GET'],
              uris: ['/uuid'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 80, priority: 0, weight: 1 }],
            scheme: 'http',
          },
        },
        resourceId: ADCSDK.utils.generateId('HTTPBIN Service1'),
        resourceName: 'HTTPBIN Service1',
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.CREATE,
      },
      {
        newValue: { methods: ['GET'], name: 'Anything', uris: ['/anything'] },
        parentId: ADCSDK.utils.generateId('HTTPBIN Service1'),
        resourceId: ADCSDK.utils.generateId('HTTPBIN Service1.Anything'),
        resourceName: 'Anything',
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.CREATE,
      },
      {
        newValue: { methods: ['GET'], name: 'Generate UUID', uris: ['/uuid'] },
        parentId: ADCSDK.utils.generateId('HTTPBIN Service1'),
        resourceId: ADCSDK.utils.generateId('HTTPBIN Service1.Generate UUID'),
        resourceName: 'Generate UUID',
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.CREATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should selectively merge the objects in default values, on a service', () => {
    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: 'Test Service',
              upstream: {
                nodes: [
                  {
                    host: 'httpbin.org',
                    port: 80,
                    weight: 100,
                  },
                ],
              },
              routes: [
                {
                  name: 'anything',
                  uris: ['/anything'],
                },
              ],
            },
          ],
        },
        {
          services: [
            {
              id: ADCSDK.utils.generateId('Test Service'),
              name: 'Test Service',
              upstream: {
                name: 'default',
                scheme: 'http',
                type: 'roundrobin',
                hash_on: 'vars',
                nodes: [
                  {
                    host: 'httpbin.org',
                    port: 80,
                    weight: 100,
                    priority: 0,
                  },
                ],
                retry_timeout: 0,
                pass_host: 'pass',
              },
              routes: [
                {
                  id: ADCSDK.utils.generateId('Test Service.anything'),
                  name: 'anything',
                  uris: ['/anything'],
                },
              ],
            },
          ],
        },
        {
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
                pass_host: 'pass',
                retry_timeout: 0,
                scheme: 'http',
                timeout: { connect: 60, read: 60, send: 60 },
                type: 'roundrobin',
                nodes: [{ priority: 0 }],
              },
            },
          },
        },
      ),
    ).toEqual([]);
  });
});
