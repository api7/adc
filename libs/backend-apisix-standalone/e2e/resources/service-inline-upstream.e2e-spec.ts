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

  afterEach(() => vi.useRealTimers());

  it('Initialize cache', () =>
    expect(dumpConfiguration(backend)).resolves.not.toThrow());

  const serviceName = 'test';
  const config = {
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
  };
  it('Create service', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(100);
    const events = DifferV3.diff(config, await dumpConfiguration(backend));
    return syncEvents(backend, events);
  });

  it('Check configuration', () => {
    const rawConfig = rawConfigCache.get(cacheKey);
    expect(rawConfig?.services?.[0].id).toEqual(
      ADCSDK.utils.generateId(serviceName),
    );
    expect(rawConfig?.services?.[0].modifiedIndex).toEqual(100);
    expect(rawConfig?.upstreams).not.toBeUndefined();
    expect(rawConfig?.upstreams).toHaveLength(1);
    expect(rawConfig?.upstreams?.[0].id).toEqual(
      ADCSDK.utils.generateId(serviceName),
    );
    expect(rawConfig?.upstreams?.[0].name).toEqual(serviceName);
    expect(rawConfig?.upstreams?.[0].modifiedIndex).toEqual(100);
    expect(rawConfig?.services_conf_version).toEqual(100);
    expect(rawConfig?.upstreams_conf_version).toEqual(100);
    expect(rawConfig?.consumers_conf_version).toBeUndefined();
    expect(rawConfig?.global_rules_conf_version).toBeUndefined();
    expect(rawConfig?.plugin_metadata_conf_version).toBeUndefined();
    expect(rawConfig?.routes_conf_version).toBeUndefined();
    expect(rawConfig?.ssls_conf_version).toBeUndefined();
    expect(rawConfig?.stream_routes_conf_version).toBeUndefined();

    const config = configCache.get(cacheKey);
    expect(config?.services).not.toBeUndefined();
    expect(config?.services?.[0].upstream).not.toBeUndefined();
  });

  it('Update inlined upstream', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(200);

    const newConfig = structuredClone(config);
    newConfig.services[0].upstream.nodes[0].port = 19080;

    const events = DifferV3.diff(newConfig, await dumpConfiguration(backend));
    expect(events).toHaveLength(1);
    expect(events[0].type).toEqual(ADCSDK.EventType.UPDATE);
    expect(events[0].resourceType).toEqual(ADCSDK.ResourceType.SERVICE);
    expect(events[0].diff?.[0].path?.[0]).toEqual('upstream');

    return syncEvents(backend, events);
  });

  it('Check configuration', () => {
    const rawConfig = rawConfigCache.get(cacheKey);
    expect(rawConfig?.upstreams?.[0].modifiedIndex).toEqual(200);
    expect(rawConfig?.services?.[0].modifiedIndex).toEqual(100);
    expect(rawConfig?.upstreams_conf_version).toEqual(200);
    expect(rawConfig?.services_conf_version).toEqual(100);
    expect(rawConfig?.consumers_conf_version).toBeUndefined();
    expect(rawConfig?.global_rules_conf_version).toBeUndefined();
    expect(rawConfig?.plugin_metadata_conf_version).toBeUndefined();
    expect(rawConfig?.routes_conf_version).toBeUndefined();
    expect(rawConfig?.ssls_conf_version).toBeUndefined();
    expect(rawConfig?.stream_routes_conf_version).toBeUndefined();
  });

  it('Update inlined upstream again', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(300);

    const newConfig = structuredClone(config);
    newConfig.services[0].upstream.nodes = [];

    const events = DifferV3.diff(newConfig, await dumpConfiguration(backend));
    expect(events).toHaveLength(1);
    expect(events[0].type).toEqual(ADCSDK.EventType.UPDATE);
    expect(events[0].resourceType).toEqual(ADCSDK.ResourceType.SERVICE);
    expect(events[0].diff?.[0].path?.[0]).toEqual('upstream');

    return syncEvents(backend, events);
  });

  it('Check configuration', () => {
    const rawConfig = rawConfigCache.get(cacheKey);
    expect(rawConfig?.upstreams?.[0].modifiedIndex).toEqual(300);
    expect(rawConfig?.upstreams?.[0].nodes).toHaveLength(0);
    expect(rawConfig?.services?.[0].modifiedIndex).toEqual(100);
    expect(rawConfig?.upstreams_conf_version).toEqual(300);
    expect(rawConfig?.services_conf_version).toEqual(100);
    expect(rawConfig?.consumers_conf_version).toBeUndefined();
    expect(rawConfig?.global_rules_conf_version).toBeUndefined();
    expect(rawConfig?.plugin_metadata_conf_version).toBeUndefined();
    expect(rawConfig?.routes_conf_version).toBeUndefined();
    expect(rawConfig?.ssls_conf_version).toBeUndefined();
    expect(rawConfig?.stream_routes_conf_version).toBeUndefined();
  });

  it('Delete service', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(400);

    const events = DifferV3.diff({}, await dumpConfiguration(backend));
    expect(events).toHaveLength(1);
    expect(events[0].type).toEqual(ADCSDK.EventType.DELETE);
    expect(events[0].resourceType).toEqual(ADCSDK.ResourceType.SERVICE);

    return syncEvents(backend, events);
  });

  it('Check configuration', () => {
    const rawConfig = rawConfigCache.get(cacheKey);
    expect(rawConfig?.upstreams).toHaveLength(0);
    expect(rawConfig?.services).toHaveLength(0);
    expect(rawConfig?.upstreams_conf_version).toEqual(400);
    expect(rawConfig?.services_conf_version).toEqual(400);
  });
});
