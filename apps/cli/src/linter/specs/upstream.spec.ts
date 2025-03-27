import * as ADCSDK from '@api7/adc-sdk';

import { check } from '../';

describe('Upstream Linter', () => {
  const cases = [
    {
      name: 'should check either nodes or discovery (neither)',
      input: {
        services: [
          {
            name: 'No_Node_And_Discovery',
            upstream: {},
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'custom',
          message:
            'Upstream must either explicitly specify nodes or use service discovery and not both',
          path: ['services', 0, 'upstream'],
        },
      ],
    },
    {
      name: 'should check either nodes or discovery (with nodes)',
      input: {
        services: [
          {
            name: 'No_Node_And_Discovery',
            upstream: {
              nodes: [
                {
                  host: '1.1.1.1',
                  port: 443,
                  weight: 100,
                },
              ],
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should check either nodes or discovery (with discovery and service name)',
      input: {
        services: [
          {
            name: 'No_Node_And_Discovery',
            upstream: {
              discovery_type: 'mock',
              service_name: 'service_mock',
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should check active health checker optional field',
      input: {
        services: [
          {
            name: 'No_HealthChecker_HostPort',
            upstream: {
              nodes: [
                {
                  host: '1.1.1.1',
                  port: 443,
                  weight: 100,
                },
              ],
              checks: {
                active: {
                  type: 'http',
                  http_path: '/',
                  healthy: {
                    interval: 2,
                    successes: 1,
                  },
                  unhealthy: {
                    interval: 1,
                    timeouts: 3,
                  },
                },
              },
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should only allow upstream mtls in client_cert and client_key',
      input: {
        services: [
          {
            name: 'Upstream mTLS',
            upstream: {
              nodes: [
                {
                  host: '1.1.1.1',
                  port: 443,
                  weight: 100,
                },
              ],
              tls: {
                client_cert: '0000',
                client_key: '0000',
              },
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should ensure name is required in upstreams, and id is allowed',
      input: {
        services: [
          {
            name: 'test',
            upstreams: [
              {
                id: 'my-upstream',
                nodes: [
                  {
                    host: '1.1.1.1',
                    port: 443,
                    weight: 100,
                  },
                ],
                tls: {
                  client_cert: '0000',
                  client_key: '0000',
                },
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'invalid_type',
          expected: 'string',
          message: 'Required',
          path: ['services', 0, 'upstreams', 0, 'name'],
          received: 'undefined',
        },
      ],
    },
  ];

  // test cases runner
  cases.forEach((item) => {
    it(item.name, () => {
      const result = check(item.input);
      expect(result.success).toEqual(item.expect);
      if (!item.expect) {
        expect(result.error.errors).toEqual(item.errors);
      }
    });
  });
});
