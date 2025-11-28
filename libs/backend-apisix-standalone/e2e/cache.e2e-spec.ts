import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';
import { gt, lte } from 'semver';

import { BackendAPISIXStandalone } from '../src';
import {
  config as configCache,
  latestVersion as latestVersionCache,
  rawConfig as rawConfigCache,
} from '../src/cache';
import { stableTimestamp } from '../src/utils';
import {
  defaultBackendOptions,
  server1,
  server2,
  server3,
  servers,
  token1,
  token2,
  tokens,
} from './support/constants';
import {
  conditionalIt,
  dumpConfiguration,
  restartAPISIX,
  semverCondition,
  syncEvents,
  wait,
} from './support/utils';

vi.mock('../src/utils', () => ({
  stableTimestamp: vi.fn(),
}));

const mockStableTimestamp = vi.mocked(stableTimestamp);

describe('Cache - Single APISIX', () => {
  let backend: BackendAPISIXStandalone;

  const cacheKey = 'default';
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

  afterAll(() => (configCache.clear(), rawConfigCache.clear()));

  it('initialize cache (load from fresh new APISIX instance)', async () => {
    expect(configCache).toBeDefined();
    expect(configCache.size).toEqual(0);
    expect(rawConfigCache).toBeDefined();
    expect(rawConfigCache.size).toEqual(0);

    await expect(dumpConfiguration(backend)).resolves.not.toThrow();

    expect(configCache.size).toEqual(1);
    expect(rawConfigCache.size).toEqual(1);
    // APISIX does not report when it was last updated (it has not yet gotten configured),
    // so fetcher will automatically populate empty objects as a default.
    expect(configCache.get(cacheKey)).toMatchObject({
      services: [],
      ssls: [],
      consumers: [],
      global_rules: {},
      plugin_metadata: {},
    });
    Object.values(
      rawConfigCache.get(cacheKey) as Record<string, unknown>,
    ).forEach((item) => expect(item).toEqual(0));
  });

  it('dump again (should use cache)', async () => {
    expect(configCache.size).toEqual(1);

    let apiCall = 0;
    const sub = backend.on('AXIOS_DEBUG', () => apiCall++);

    await expect(dumpConfiguration(backend)).resolves.not.toThrow();
    expect(apiCall).toEqual(0);

    sub.unsubscribe();
  });

  const config = {
    services: [
      {
        name: 'service1',
        upstream: { nodes: [{ host: '127.0.0.1', port: 9180, weight: 100 }] },
        routes: [
          {
            name: 'route1',
            uris: ['/apisix/admin/configs'],
          },
        ],
      },
    ],
    consumers: [
      { username: 'jack', plugins: {} },
      { username: 'jane', plugins: {} },
    ],
  } as ADCSDK.Configuration;
  const now = new Date();
  mockStableTimestamp.mockReset();
  mockStableTimestamp.mockReturnValue(now.getTime());
  it('update config', async () => {
    const oldConfig = await dumpConfiguration(backend);
    const events = DifferV3.diff(config, oldConfig);
    expect(events).toHaveLength(1 + 1 + 2); // service * 1 + route * 1 + consumer * 2
    expect(
      events
        .map((item) => item.type)
        .filter((item) => item === ADCSDK.EventType.CREATE),
    ).toHaveLength(4);

    const result = await syncEvents(backend, events);
    expect(result[0].server).toEqual(server1);

    return result;
  });

  it('check if the cache is updated', async () => {
    expect(configCache.get(cacheKey)).toMatchObject(config);

    const timestamp = now.getTime();
    expect(rawConfigCache.get(cacheKey)).toMatchObject({
      services: [
        {
          id: ADCSDK.utils.generateId('service1'),
          modifiedIndex: timestamp,
          name: 'service1',
          upstream_id: ADCSDK.utils.generateId('service1'),
        },
      ],
      services_conf_version: timestamp,
      upstreams: [
        {
          id: ADCSDK.utils.generateId('service1'),
          name: 'service1',
          modifiedIndex: timestamp,
          nodes: [
            {
              host: '127.0.0.1',
              port: 9180,
              weight: 100,
            },
          ],
        },
      ],
      upstreams_conf_version: timestamp,
      routes: [
        {
          id: ADCSDK.utils.generateId('service1.route1'),
          modifiedIndex: timestamp,
          name: 'route1',
          service_id: ADCSDK.utils.generateId('service1'),
          status: 1,
          uris: ['/apisix/admin/configs'],
        },
      ],
      routes_conf_version: timestamp,
      consumers: [
        {
          modifiedIndex: timestamp,
          plugins: {},
          username: 'jack',
        },
        {
          modifiedIndex: timestamp,
          plugins: {},
          username: 'jane',
        },
      ],
      consumers_conf_version: timestamp,
    });
    expect(latestVersionCache.get(cacheKey)).toEqual(timestamp);
  });

  it('wait for sync', async () => wait(100));

  it('check route', async () => {
    const res = await axios.get('http://127.0.0.1:19080/apisix/admin/configs', {
      validateStatus: () => true,
    });
    expect(res.status).toEqual(401);
  });
});

// All APISIX instances are new, i.e., no configuration has been loaded for each instance
// The ADC will synchronize resource creation to all instances, starting with an empty configuration
describe('Cache - Multiple APISIX (completely new instances)', () => {
  let backend: BackendAPISIXStandalone;

  const cacheKey = 'default';
  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server: servers,
      token: tokens,
      tlsSkipVerify: true,
      cacheKey,
      ...defaultBackendOptions,
    });
  });

  afterAll(() => (configCache.clear(), rawConfigCache.clear()));

  it('initialize cache', async () => {
    expect(configCache).toBeDefined();
    expect(configCache.size).toEqual(0);
    expect(rawConfigCache).toBeDefined();
    expect(rawConfigCache.size).toEqual(0);

    await expect(dumpConfiguration(backend)).resolves.not.toThrow();

    expect(configCache.size).toEqual(1);
    expect(rawConfigCache.size).toEqual(1);
    // APISIX does not report when it was last updated (it has not yet gotten configured),
    // so fetcher will automatically populate empty objects as a default.
    expect(configCache.get(cacheKey)).toMatchObject({
      services: [],
      ssls: [],
      consumers: [],
      global_rules: {},
      plugin_metadata: {},
    });
    Object.values(
      rawConfigCache.get(cacheKey) as Record<string, unknown>,
    ).forEach((item) => expect(item).toEqual(0));
  });

  const config = {
    services: [
      {
        name: 'service1',
        upstream: { nodes: [{ host: '127.0.0.1', port: 9180, weight: 100 }] },
        routes: [
          {
            name: 'route1',
            uris: ['/apisix/admin/configs'],
          },
        ],
      },
    ],
    consumers: [
      { username: 'jack', plugins: {} },
      { username: 'jane', plugins: {} },
    ],
  } as ADCSDK.Configuration;
  it('update config', async () => {
    const oldConfig = await dumpConfiguration(backend);
    const events = DifferV3.diff(config, oldConfig);
    expect(events).toHaveLength(1 + 1 + 2); // service * 1 + route * 1 + consumer * 2
    expect(
      events
        .map((item) => item.type)
        .filter((item) => item === ADCSDK.EventType.CREATE),
    ).toHaveLength(4);

    const result = await syncEvents(backend, events);
    result.forEach((item) => {
      expect([server1, server2, server3].includes(`${item.server}`)).toEqual(
        true,
      );
    });

    return result;
  });

  it('wait for sync', async () => wait(100));

  it('check routes', async () => {
    const res1 = await axios.get(
      'http://127.0.0.1:19080/apisix/admin/configs',
      {
        validateStatus: () => true,
      },
    );
    expect(res1.status).toEqual(401);
    const res2 = await axios.get(
      'http://127.0.0.1:29080/apisix/admin/configs',
      {
        validateStatus: () => true,
      },
    );
    expect(res2.status).toEqual(401);
    const res3 = await axios.get(
      'http://127.0.0.1:39080/apisix/admin/configs',
      {
        validateStatus: () => true,
      },
    );
    expect(res3.status).toEqual(401);
  });
});

// Only a subset of APISIX instances are new, and the cluster contains one or more instances that have received configuration
// The ADC will restore the cache from the last synchronized instance and diff there.
describe('Cache - Multiple APISIX (Partial new instances)', () => {
  let backend: BackendAPISIXStandalone;

  const cacheKey = 'default';
  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server: servers,
      token: tokens,
      tlsSkipVerify: true,
      cacheKey,
      ...defaultBackendOptions,
    });
  });

  it('send config to instance 1', async () => {
    const config = {
      services: [
        {
          name: 'service1',
          upstream: { nodes: [{ host: '127.0.0.1', port: 5432, weight: 100 }] },
          routes: [
            {
              name: 'route1',
              uris: ['/apisix/admin/configs'],
            },
          ],
        },
      ],
    } as ADCSDK.Configuration;
    const events = DifferV3.diff(config, {});
    latestVersionCache.clear();
    configCache.set(cacheKey, {});
    rawConfigCache.set(cacheKey, {});
    expect(events).toHaveLength(1 + 1); // service * 1 + route * 1
    expect(
      events
        .map((item) => item.type)
        .filter((item) => item === ADCSDK.EventType.CREATE),
    ).toHaveLength(2);

    mockStableTimestamp.mockReset();
    mockStableTimestamp.mockReturnValue(100);

    return syncEvents(
      new BackendAPISIXStandalone({
        server: server1,
        token: token1,
        tlsSkipVerify: true,
        cacheKey,
        ...defaultBackendOptions,
      }),
      events,
    );
  });

  it('wait for sync', () => wait(1000));

  it('send config to instance 2', async () => {
    const config = {
      services: [
        {
          name: 'service1',
          upstream: { nodes: [{ host: '127.0.0.1', port: 3306, weight: 100 }] },
          routes: [
            {
              name: 'route1',
              uris: ['/apisix/admin/configs'],
            },
          ],
        },
      ],
    } as ADCSDK.Configuration;
    configCache.set(cacheKey, {});
    rawConfigCache.set(cacheKey, {});
    const events = DifferV3.diff(config, {});
    expect(events).toHaveLength(1 + 1); // service * 1 + route * 1
    expect(
      events
        .map((item) => item.type)
        .filter((item) => item === ADCSDK.EventType.CREATE),
    ).toHaveLength(2);

    mockStableTimestamp.mockReset();
    mockStableTimestamp.mockReturnValue(200);

    return syncEvents(
      new BackendAPISIXStandalone({
        server: server2,
        token: token2,
        tlsSkipVerify: true,
        cacheKey,
        ...defaultBackendOptions,
      }),
      events,
    );
  });

  conditionalIt(semverCondition(lte, '3.13.0'))(
    'initialize cache (<= 3.13.0)',
    async () => {
      configCache.clear();
      rawConfigCache.clear();
      expect(configCache).toBeDefined();
      expect(configCache.size).toEqual(0);
      expect(rawConfigCache).toBeDefined();
      expect(rawConfigCache.size).toEqual(0);

      await expect(dumpConfiguration(backend)).resolves.not.toThrow();

      expect(configCache.size).toEqual(1);
      expect(rawConfigCache.size).toEqual(1);
    },
  );

  conditionalIt(semverCondition(gt, '3.13.0'))(
    'initialize cache (> 3.13.0)',
    async () => {
      configCache.clear();
      rawConfigCache.clear();
      expect(configCache).toBeDefined();
      expect(configCache.size).toEqual(0);
      expect(rawConfigCache).toBeDefined();
      expect(rawConfigCache.size).toEqual(0);

      await expect(dumpConfiguration(backend)).resolves.not.toThrow();

      expect(configCache.size).toEqual(1);
      expect(rawConfigCache.size).toEqual(1);

      const rawConfig = rawConfigCache.get(cacheKey);
      expect(rawConfig?.services_conf_version).toEqual(200);
      expect(rawConfig?.routes_conf_version).toEqual(200);
      expect(rawConfig?.upstreams_conf_version).toEqual(200); // inline upstream caused conf version increase

      expect(rawConfig?.plugin_metadata_conf_version).toBeLessThan(200);
      expect(rawConfig?.global_rules_conf_version).toBeLessThan(200);
      expect(rawConfig?.consumers_conf_version).toBeLessThan(200);
      expect(rawConfig?.ssls_conf_version).toBeLessThan(200);
    },
  );
});
