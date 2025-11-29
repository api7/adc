import { Differ } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPISIXStandalone } from '../../src';
import {
  config as configCache,
  rawConfig as rawConfigCache,
} from '../../src/cache';
import * as typing from '../../src/typing';
import { defaultBackendOptions, server1, token1 } from '../support/constants';
import { dumpConfiguration, restartAPISIX, syncEvents } from '../support/utils';

const cacheKey = 'default';
describe('Service-Upstreams E2E', () => {
  let backend: BackendAPISIXStandalone;

  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server: server1,
      token: token1,
      tlsSkipVerify: true,
      cacheKey,
      ...defaultBackendOptions,
    });
  });

  describe('Sync and dump service with multiple upstreams', () => {
    const upstreamND1Name = 'nd-upstream1';
    const upstreamND1 = {
      name: upstreamND1Name,
      type: 'roundrobin',
      scheme: 'https',
      nodes: [
        {
          host: '1.1.1.1',
          port: 443,
          weight: 100,
        },
      ],
    } satisfies ADCSDK.Upstream;
    const upstreamND2Name = 'nd-upstream2';
    const upstreamND2 = {
      //@ts-expect-error custom id
      id: upstreamND2Name,
      name: upstreamND2Name,
      type: 'roundrobin',
      scheme: 'https',
      nodes: [
        {
          host: '1.0.0.1',
          port: 443,
          weight: 100,
        },
      ],
    } satisfies ADCSDK.Upstream;
    const serviceName = 'test';
    const service = {
      name: serviceName,
      upstream: {
        type: 'roundrobin',
        nodes: [
          {
            host: 'httpbin.org',
            port: 443,
            weight: 100,
          },
        ],
      },
      upstreams: [upstreamND1, upstreamND2],
    } satisfies ADCSDK.Service;

    it('Initialize cache', () =>
      expect(dumpConfiguration(backend)).resolves.not.toThrow());

    it('Create', async () =>
      syncEvents(
        backend,
        Differ.diff(
          {
            services: [service],
          },
          await dumpConfiguration(backend),
        ),
      ));

    const checkOriginalConfig = () => {
      const rawConfig = rawConfigCache.get(cacheKey);
      expect(rawConfig?.services?.[0].id).toEqual(
        ADCSDK.utils.generateId(serviceName),
      );
      expect(rawConfig?.upstreams).not.toBeUndefined();
      expect(rawConfig?.upstreams).toHaveLength(3);
      expect(rawConfig?.upstreams?.[0].name).toEqual(serviceName);
      expect(rawConfig?.upstreams?.[1].name).toEqual(upstreamND1Name);
      expect(rawConfig?.upstreams?.[2].name).toEqual(upstreamND2Name);
      expect(rawConfig?.upstreams?.[0].labels).toBeUndefined();
      expect(
        rawConfig?.upstreams?.[1].labels?.[
          typing.ADC_UPSTREAM_SERVICE_ID_LABEL
        ],
      ).toEqual(ADCSDK.utils.generateId(serviceName));
      expect(
        rawConfig?.upstreams?.[2].labels?.[
          typing.ADC_UPSTREAM_SERVICE_ID_LABEL
        ],
      ).toEqual(ADCSDK.utils.generateId(serviceName));

      const config = configCache.get(cacheKey);
      expect(config?.services).not.toBeUndefined();
      expect(config?.services).toHaveLength(1);
      expect(config?.services?.[0].upstreams).toHaveLength(2);
      expect(
        config?.services?.[0].upstreams?.[0].labels?.[
          typing.ADC_UPSTREAM_SERVICE_ID_LABEL
        ],
      ).toBeUndefined();
      expect(
        config?.services?.[0].upstreams?.[1].labels?.[
          typing.ADC_UPSTREAM_SERVICE_ID_LABEL
        ],
      ).toBeUndefined();
    };
    it('Check cache', checkOriginalConfig);

    it('Try update (without any change)', async () =>
      syncEvents(
        backend,
        Differ.diff(
          {
            services: [service],
          },
          await dumpConfiguration(backend),
        ),
      ));

    it('Check cache 2', checkOriginalConfig);

    it('Try update', async () => {
      const newService = structuredClone(service);
      newService.upstreams[0].nodes[0].host = '8.8.8.8';
      await syncEvents(
        backend,
        Differ.diff(
          {
            services: [newService],
          },
          await dumpConfiguration(backend),
        ),
      );
    });

    it('Check updated cache', () => {
      const rawConfig = rawConfigCache.get(cacheKey);
      expect(rawConfig?.services?.[0].id).toEqual(
        ADCSDK.utils.generateId(serviceName),
      );
      expect(rawConfig?.upstreams).not.toBeUndefined();
      expect(rawConfig?.upstreams).toHaveLength(3);
      expect(
        rawConfig?.upstreams?.[1].labels?.[
          typing.ADC_UPSTREAM_SERVICE_ID_LABEL
        ],
      ).toEqual(ADCSDK.utils.generateId(serviceName));
      expect(rawConfig?.upstreams?.[1].nodes?.[0].host).toEqual('8.8.8.8');

      const config = configCache.get(cacheKey);
      expect(config?.services).not.toBeUndefined();
      expect(config?.services).toHaveLength(1);
      expect(config?.services?.[0].upstreams).toHaveLength(2);
      expect(
        config?.services?.[0].upstreams?.[0].labels?.[
          typing.ADC_UPSTREAM_SERVICE_ID_LABEL
        ],
      ).toBeUndefined();
      expect(config?.services?.[0].upstreams?.[0].nodes?.[0].host).toEqual(
        '8.8.8.8',
      );
    });
  });
});
