import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';

import { BackendAPISIXStandalone } from '../src';
import {
  config as configCache,
  rawConfig as rawConfigCache,
} from '../src/cache';
import { server, token } from './support/constants';
import { dumpConfiguration, restartAPISIX, syncEvents } from './support/utils';

describe('Cache - Single APISIX', () => {
  let backend: BackendAPISIXStandalone;

  const cacheKey = 'default';
  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server,
      token,
      tlsSkipVerify: true,
      cacheKey,
    });
  });

  beforeEach(() => vi.useFakeTimers());

  afterEach(() => vi.useRealTimers());

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
    expect(configCache.get(cacheKey)).toEqual({});
    expect(rawConfigCache.get(cacheKey)).toEqual({});
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
  it('update config', async () => {
    const oldConfig = await dumpConfiguration(backend);
    const events = DifferV3.diff(config, oldConfig);
    expect(events).toHaveLength(1 + 1 + 2); // service * 1 + route * 1 + consumer * 2
    expect(
      events
        .map((item) => item.type)
        .filter((item) => item === ADCSDK.EventType.CREATE),
    ).toHaveLength(4);

    vi.setSystemTime(now);

    return syncEvents(backend, events);
  });

  it('check if the cache is updated', async () => {
    expect(configCache.get(cacheKey)).toMatchObject(config);

    const timestamp = Math.ceil(now.getTime() / 1000);
    expect(rawConfigCache.get(cacheKey)).toMatchObject({
      services: [
        {
          id: ADCSDK.utils.generateId('service1'),
          modifiedIndex: timestamp,
          name: 'service1',
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
      services_conf_version: timestamp,
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
  });

  it('check route', async () => {
    const res = await axios.get('http://127.0.0.1:19080/apisix/admin/configs', {
      validateStatus: () => true,
    });
    expect(res.status).toEqual(401);
  });
});
