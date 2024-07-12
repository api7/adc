import * as ADCSDK from '@api7/adc-sdk';
import { unset } from 'lodash';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { BackendAPISIX } from '../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
  updateEvent,
} from './support/utils';

describe('Sync and Dump - 1', () => {
  let backend: BackendAPISIX;

  beforeAll(() => {
    backend = new BackendAPISIX({
      server: globalThis.server,
      token: globalThis.token,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  describe('Sync and dump empty service', () => {
    const upstream = {
      scheme: 'https',
      nodes: [
        {
          host: 'httpbin.org',
          port: 443,
          weight: 100,
        },
      ],
    };
    const service1Name = 'service1';
    const service1 = {
      name: service1Name,
      upstream: structuredClone(upstream),
    } as ADCSDK.Service;
    const service2Name = 'service2';
    const service2 = {
      name: service2Name,
      upstream: structuredClone(upstream),
    } as ADCSDK.Service;

    it('Create services', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
        createEvent(ADCSDK.ResourceType.SERVICE, service2Name, service2),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(2);
      expect(result.services[0]).toMatchObject(service2);
      expect(result.services[1]).toMatchObject(service1);
    });

    it('Update service1', async () => {
      service1.description = 'desc';
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
      ]);
    });

    it('Dump again (service1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services[1]).toMatchObject(service1);
    });

    it('Delete service1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, service1Name),
      ]));

    it('Dump again (service1 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service2);
    });

    it('Delete service2', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, service2Name),
      ]));

    it('Dump again (service2 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });

  describe('Sync and dump service with routes', () => {
    const serviceName = 'test';
    const service = {
      name: serviceName,
      upstream: {
        scheme: 'https',
        nodes: [
          {
            host: 'httpbin.org',
            port: 443,
            weight: 100,
          },
        ],
      },
    } as ADCSDK.Service;
    const route1Name = 'route1';
    const route1 = {
      name: route1Name,
      uris: ['/route1'],
    } as ADCSDK.Route;
    const route2Name = 'route2';
    const route2 = {
      name: route2Name,
      uris: ['/route2'],
      plugins: {
        'key-auth': {},
      },
    } as ADCSDK.Route;

    it('Create resources', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
        createEvent(ADCSDK.ResourceType.ROUTE, route1Name, route1, serviceName),
        createEvent(ADCSDK.ResourceType.ROUTE, route2Name, route2, serviceName),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].routes).toHaveLength(2);
      expect(result.services[0].routes[0]).toMatchObject(route1);
      expect(result.services[0].routes[1]).toMatchObject(route2);
    });

    it('Delete route1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.ROUTE, route1Name, serviceName),
      ]));

    it('Dump again (check remain route2)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].routes).toHaveLength(1);
      expect(result.services[0].routes[0]).toMatchObject(route2);
    });

    it('Delete service', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.ROUTE, route2Name, serviceName),
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again (service should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });

  describe('Sync and dump consumers', () => {
    const consumer1Name = 'consumer1';
    const consumer1 = {
      username: consumer1Name,
      plugins: {
        'key-auth': {
          key: consumer1Name,
        },
      },
    } as ADCSDK.Consumer;
    const consumer2Name = 'consumer2';
    const consumer2 = {
      username: consumer2Name,
      plugins: {
        'key-auth': {
          key: consumer2Name,
        },
      },
    } as ADCSDK.Consumer;

    it('Create consumers', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name, consumer1),
        createEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name, consumer2),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(2);
      expect(result.consumers[0]).toMatchObject(consumer1);
      expect(result.consumers[1]).toMatchObject(consumer2);
    });

    it('Update consumer1', async () => {
      consumer1.description = 'desc';
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name, consumer1),
      ]);
    });

    it('Dump again (consumer1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers[0]).toMatchObject(consumer1);
    });

    it('Delete consumer1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
      ]));

    it('Dump again (consumer1 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(1);
      expect(result.consumers[0]).toMatchObject(consumer2);
    });

    it('Delete consumer2', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name),
      ]));

    it('Dump again (consumer2 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(0);
    });
  });

  describe('Sync and dump ssls', () => {
    const certificate = {
      certificate: readFileSync(
        join(__dirname, 'assets/test-ssl.cer'),
      ).toString('utf-8'),
      key: readFileSync(join(__dirname, 'assets/test-ssl.key')).toString(
        'utf-8',
      ),
    };
    const ssl1SNIs = ['ssl1-1.com', 'ssl1-2.com'];
    const ssl1 = {
      snis: ssl1SNIs,
      certificates: [certificate],
    } as ADCSDK.SSL;
    const ssl2SNIs = ['ssl2-1.com', 'ssl2-2.com'];
    const ssl2 = {
      snis: ssl2SNIs,
      certificates: [certificate],
    } as ADCSDK.SSL;
    const sslName = (snis: Array<string>) => snis.join(',');

    const ssl1test = structuredClone(ssl1);
    const ssl2test = structuredClone(ssl2);
    unset(ssl1test, 'certificates.0.key');
    unset(ssl2test, 'certificates.0.key');

    it('Create ssls', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SSL, sslName(ssl1SNIs), ssl1),
        createEvent(ADCSDK.ResourceType.SSL, sslName(ssl2SNIs), ssl2),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.ssls).toHaveLength(2);
      expect(result.ssls[0]).toMatchObject(ssl2test);
      expect(result.ssls[1]).toMatchObject(ssl1test);
    });

    it('Update ssl1', async () => {
      ssl1.labels = { test: 'test' };
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.SSL, sslName(ssl1SNIs), ssl1),
      ]);
    });

    it('Dump again (ssl1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      const testCase = structuredClone(ssl1);
      unset(testCase, 'certificates.0.key');
      expect(result.ssls[1]).toMatchObject(testCase);
    });

    it('Delete ssl1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SSL, sslName(ssl1SNIs)),
      ]));

    it('Dump again (ssl1 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.ssls).toHaveLength(1);
      expect(result.ssls[0]).toMatchObject(ssl2test);
    });

    it('Delete ssl2', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SSL, sslName(ssl2SNIs)),
      ]));

    it('Dump again (ssl2 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.ssls).toHaveLength(0);
    });
  });

  describe('Sync and dump global rules', () => {
    const plugin1Name = 'prometheus';
    const plugin1 = {
      prefer_name: true,
    } as ADCSDK.GlobalRule;
    const plugin2Name = 'file-logger';
    const plugin2 = {
      path: 'logs/file.log',
    } as ADCSDK.GlobalRule;

    it('Create global rules', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.GLOBAL_RULE, plugin1Name, plugin1),
        createEvent(ADCSDK.ResourceType.GLOBAL_RULE, plugin2Name, plugin2),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.global_rules)).toHaveLength(2);
      expect(result.global_rules[plugin1Name]).toMatchObject(plugin1);
      expect(result.global_rules[plugin2Name]).toMatchObject(plugin2);
    });

    it('Update plugin1', async () => {
      plugin1.test = 'test';
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.GLOBAL_RULE, plugin1Name, plugin1),
      ]);
    });

    it('Dump again (plugin1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.global_rules[plugin1Name]).toMatchObject(plugin1);
    });

    it('Delete plugin1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.GLOBAL_RULE, plugin1Name),
      ]));

    it('Dump again (plugin1 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.global_rules)).toHaveLength(1);
      expect(result.global_rules[plugin1Name]).toBeUndefined();
      expect(result.global_rules[plugin2Name]).toMatchObject(plugin2);
    });

    it('Delete plugin2', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.GLOBAL_RULE, plugin2Name),
      ]));

    it('Dump again (plugin2 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.global_rules)).toHaveLength(0);
    });
  });

  describe('Sync and dump plugin metadata', () => {
    const plugin1Name = 'http-logger';
    const plugin1 = {
      log_format: { test: 'test', test1: 'test1' },
    } as ADCSDK.PluginMetadata;
    const plugin2Name = 'tcp-logger';
    const plugin2 = {
      log_format: { test: 'test', test1: 'test1' },
    } as ADCSDK.PluginMetadata;

    it('Create plugin metadata', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.PLUGIN_METADATA, plugin1Name, plugin1),
        createEvent(ADCSDK.ResourceType.PLUGIN_METADATA, plugin2Name, plugin2),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.plugin_metadata)).toHaveLength(2);
      expect(result.plugin_metadata[plugin1Name]).toMatchObject(plugin1);
      expect(result.plugin_metadata[plugin2Name]).toMatchObject(plugin2);
    });

    it('Update plugin1', async () => {
      plugin1.test = 'test';
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.PLUGIN_METADATA, plugin1Name, plugin1),
      ]);
    });

    it('Dump again (plugin1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.plugin_metadata[plugin1Name]).toMatchObject(plugin1);
    });

    it('Delete plugin1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.PLUGIN_METADATA, plugin1Name),
      ]));

    it('Dump again (plugin1 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.plugin_metadata)).toHaveLength(1);
      expect(result.plugin_metadata[plugin1Name]).toBeUndefined();
      expect(result.plugin_metadata[plugin2Name]).toMatchObject(plugin2);
    });

    it('Delete plugin2', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.PLUGIN_METADATA, plugin2Name),
      ]));

    it('Dump again (plugin2 should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.plugin_metadata)).toHaveLength(0);
    });
  });
});
