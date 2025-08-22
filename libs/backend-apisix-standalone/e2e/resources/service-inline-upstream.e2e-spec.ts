import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPISIXStandalone } from '../../src';
import {
  config as configCache,
  rawConfig as rawConfigCache,
} from '../../src/cache';
import { server1, token1 } from '../support/constants';
import { dumpConfiguration, restartAPISIX, syncEvents } from '../support/utils';

const cacheKey = 'default';
describe('Service E2E - inline upstream', () => {
  let backend: BackendAPISIXStandalone;

  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server: server1,
      token: token1,
      cacheKey,
    });
  });

  it('Initialize cache', () =>
    expect(dumpConfiguration(backend)).resolves.not.toThrow());

  const serviceName = 'test';
  it('Create service', async () => {
    const events = DifferV3.diff(
      {
        services: [
          {
            name: serviceName,
            upstream: {
              nodes: [
                {
                  host: '127.0.0.1',
                  port: 9180,
                  weight: 100,
                },
              ],
            },
          },
        ],
      },
      await dumpConfiguration(backend),
    );
    return syncEvents(backend, events);
  });

  it('Check configuration', () => {
    expect(rawConfigCache.get(cacheKey)?.upstreams).not.toBeUndefined();
    expect(rawConfigCache.get(cacheKey)!.upstreams).toHaveLength(1);
    expect(rawConfigCache.get(cacheKey)!.upstreams![0].id).toEqual(
      ADCSDK.utils.generateId(serviceName),
    );
    expect(rawConfigCache.get(cacheKey)!.upstreams![0].name).toEqual(
      serviceName,
    );

    expect(configCache.get(cacheKey)?.services).not.toBeUndefined();
    expect(
      configCache.get(cacheKey)?.services![0].upstream,
    ).not.toBeUndefined();
  });
});
