import * as ADCSDK from '@api7/adc-sdk';
import { gte } from 'semver';
import { globalAgent as httpAgent } from 'node:http';

import { BackendAPI7 } from '../src';
import {
  conditionalDescribe,
  generateHTTPSAgent,
  semverCondition,
} from './support/utils';

conditionalDescribe(semverCondition(gte, '3.9.10'))(
  'Validate',
  () => {
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

    it('should report supportValidate as true', async () => {
      expect(await backend.supportValidate()).toBe(true);
    });

    it('should succeed with empty configuration', async () => {
      const result = await backend.validate({});
      expect(result.success).toBe(true);
      expect(result.errors).toEqual([]);
    });

    it('should succeed with valid service and route', async () => {
      const config: ADCSDK.Configuration = {
        services: [
          {
            name: 'validate-test-svc',
            upstream: {
              scheme: 'http',
              nodes: [{ host: 'httpbin.org', port: 80, weight: 100 }],
            },
            routes: [
              {
                name: 'validate-test-route',
                uris: ['/validate-test'],
                methods: ['GET'],
              },
            ],
          },
        ],
      };

      const result = await backend.validate(config);
      expect(result.success).toBe(true);
      expect(result.errors).toEqual([]);
    });

    it('should succeed with valid consumer', async () => {
      const config: ADCSDK.Configuration = {
        consumers: [
          {
            username: 'validate-test-consumer',
            plugins: {
              'key-auth': { key: 'test-key-123' },
            },
          },
        ],
      };

      const result = await backend.validate(config);
      expect(result.success).toBe(true);
      expect(result.errors).toEqual([]);
    });

    it('should fail with invalid plugin configuration', async () => {
      const config: ADCSDK.Configuration = {
        services: [
          {
            name: 'validate-bad-plugin-svc',
            upstream: {
              scheme: 'http',
              nodes: [{ host: 'httpbin.org', port: 80, weight: 100 }],
            },
            routes: [
              {
                name: 'validate-bad-plugin-route',
                uris: ['/bad-plugin'],
                plugins: {
                  'limit-count': {
                    // missing required fields: count, time_window
                  },
                },
              },
            ],
          },
        ],
      };

      const result = await backend.validate(config);
      expect(result.success).toBe(false);
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0].resource_type).toBe('routes');
    });

    it('should fail with invalid route (bad uri type)', async () => {
      const config: ADCSDK.Configuration = {
        services: [
          {
            name: 'validate-bad-route-svc',
            upstream: {
              scheme: 'http',
              nodes: [{ host: 'httpbin.org', port: 80, weight: 100 }],
            },
            routes: [
              {
                name: 'validate-bad-route',
                // paths should be an array of strings, provide number instead
                uris: [123 as unknown as string],
              },
            ],
          },
        ],
      };

      const result = await backend.validate(config);
      expect(result.success).toBe(false);
      expect(result.errors.length).toBeGreaterThan(0);
    });

    it('should collect multiple errors', async () => {
      const config: ADCSDK.Configuration = {
        services: [
          {
            name: 'validate-multi-err-svc',
            upstream: {
              scheme: 'http',
              nodes: [{ host: 'httpbin.org', port: 80, weight: 100 }],
            },
            routes: [
              {
                name: 'validate-multi-err-route1',
                uris: ['/multi-err-1'],
                plugins: {
                  'limit-count': {},
                },
              },
              {
                name: 'validate-multi-err-route2',
                uris: ['/multi-err-2'],
                plugins: {
                  'limit-count': {},
                },
              },
            ],
          },
        ],
      };

      const result = await backend.validate(config);
      expect(result.success).toBe(false);
      expect(result.errors.length).toBeGreaterThanOrEqual(2);
    });

    it('should succeed with mixed resource types', async () => {
      const config: ADCSDK.Configuration = {
        services: [
          {
            name: 'validate-mixed-svc',
            upstream: {
              scheme: 'https',
              nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            },
            routes: [
              {
                name: 'validate-mixed-route',
                uris: ['/mixed-test'],
                methods: ['GET', 'POST'],
              },
            ],
          },
        ],
        consumers: [
          {
            username: 'validate-mixed-consumer',
            plugins: {
              'key-auth': { key: 'mixed-key-456' },
            },
          },
        ],
        global_rules: {
          'prometheus': { prefer_name: false },
        } as ADCSDK.Configuration['global_rules'],
      };

      const result = await backend.validate(config);
      expect(result.success).toBe(true);
      expect(result.errors).toEqual([]);
    });

    it('should be a dry-run (no side effects on server)', async () => {
      const serviceName = 'validate-dryrun-svc';
      const routeName = 'validate-dryrun-route';

      const config: ADCSDK.Configuration = {
        services: [
          {
            name: serviceName,
            upstream: {
              scheme: 'http',
              nodes: [{ host: 'httpbin.org', port: 80, weight: 100 }],
            },
            routes: [
              {
                name: routeName,
                uris: ['/dryrun-test'],
              },
            ],
          },
        ],
      };

      // Validate should succeed
      const result = await backend.validate(config);
      expect(result.success).toBe(true);

      // Verify no resources were created by dumping
      const { lastValueFrom } = await import('rxjs');
      const dumped = await lastValueFrom(backend.dump());
      const found = dumped.services?.find((s) => s.name === serviceName);
      expect(found).toBeUndefined();
    });
  },
);
