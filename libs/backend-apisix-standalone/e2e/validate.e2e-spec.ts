import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { lastValueFrom } from 'rxjs';
import { gte } from 'semver';

import { BackendAPISIXStandalone } from '../src';
import {
  defaultBackendOptions,
  server1,
  token1,
} from './support/constants';
import { conditionalDescribe, semverCondition } from './support/utils';

const configToEvents = (config: ADCSDK.Configuration): Array<ADCSDK.Event> => {
  return DifferV3.diff(
    config as ADCSDK.InternalConfiguration,
    {} as ADCSDK.InternalConfiguration,
  );
};

conditionalDescribe(semverCondition(gte, '3.15.0'))('Validate', () => {
  let backend: BackendAPISIXStandalone;

  beforeAll(() => {
    backend = new BackendAPISIXStandalone({
      server: server1,
      token: token1,
      cacheKey: 'validate-test',
      ...defaultBackendOptions,
    });
  });

  it('should succeed with empty configuration', async () => {
    const result = await lastValueFrom(backend.validate([]));
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

    const result = await lastValueFrom(
      backend.validate(configToEvents(config)),
    );
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

    const result = await lastValueFrom(
      backend.validate(configToEvents(config)),
    );
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

    const result = await lastValueFrom(
      backend.validate(configToEvents(config)),
    );
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
              uris: [123 as unknown as string],
            },
          ],
        },
      ],
    };

    const result = await lastValueFrom(
      backend.validate(configToEvents(config)),
    );
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

    const result = await lastValueFrom(
      backend.validate(configToEvents(config)),
    );
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
        prometheus: { prefer_name: false },
      } as ADCSDK.Configuration['global_rules'],
    };

    const result = await lastValueFrom(
      backend.validate(configToEvents(config)),
    );
    expect(result.success).toBe(true);
    expect(result.errors).toEqual([]);
  });
});
