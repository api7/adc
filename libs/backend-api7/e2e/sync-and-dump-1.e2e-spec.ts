import * as ADCSDK from '@api7/adc-sdk';
import { unset } from 'lodash';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { BackendAPI7 } from '../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  sortResult,
  syncEvents,
  updateEvent,
} from './support/utils';

describe('Sync and Dump - 1', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: process.env.GATEWAY_GROUP,
    });
  });

  describe('Sync and dump single service', () => {
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
      result.services = sortResult(result.services, 'name');
      expect(result.services).toHaveLength(2);
      expect(result.services[0]).toMatchObject(service1);
      expect(result.services[1]).toMatchObject(service2);
    });

    it('Update service1', async () => {
      service1.description = 'desc';
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
      ]);
    });

    it('Dump again (service1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services[0]).toMatchObject(service1);
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
      path_prefix: '/test',
      strip_path_prefix: true,
    } as ADCSDK.Service;
    const route1Name = 'route1';
    const route1 = {
      name: route1Name,
      uris: ['/route1', '/route1-2'],
      priority: 100,
    } as ADCSDK.Route;
    const route2Name = 'route2';
    const route2 = {
      name: route2Name,
      uris: ['/route2', '/route2-2'],
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
      result.services[0].routes = sortResult(result.services[0].routes, 'name');

      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);

      result.services[0].routes = sortResult(result.services[0].routes, 'name');
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
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again (service should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });

  describe('Sync and dump service with stream routes', () => {
    const serviceName = 'test';
    const service = {
      name: serviceName,
      upstream: {
        scheme: 'tcp',
        nodes: [
          {
            host: '1.1.1.1',
            port: 853,
            weight: 100,
          },
        ],
      },
    } as ADCSDK.Service;
    const route1Name = 'sroute1';
    const route1 = {
      name: route1Name,
      server_port: 5432,
    } as ADCSDK.StreamRoute;
    const route2Name = 'sroute2';
    const route2 = {
      name: route2Name,
      server_port: 3306,
    } as ADCSDK.StreamRoute;
    const serviceForSync = Object.assign(structuredClone(service), {
      stream_routes: [],
    });

    it('Create resources', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, serviceForSync),
        createEvent(
          ADCSDK.ResourceType.STREAM_ROUTE,
          route1Name,
          route1,
          serviceName,
        ),
        createEvent(
          ADCSDK.ResourceType.STREAM_ROUTE,
          route2Name,
          route2,
          serviceName,
        ),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);

      result.services[0].stream_routes = sortResult(
        result.services[0].stream_routes,
        'id',
      );
      expect(result.services[0].stream_routes).toHaveLength(2);
      expect(result.services[0].stream_routes[0]).toMatchObject(route2);
      expect(result.services[0].stream_routes[1]).toMatchObject(route1);
    });

    it('Delete stream route1', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.STREAM_ROUTE, route1Name, serviceName),
      ]));

    it('Dump again (check remain stream route2)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].stream_routes).toHaveLength(1);
      expect(result.services[0].stream_routes[0]).toMatchObject(route2);
    });

    it('Delete service', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again (service should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });

  describe('Sync and dump ssls', () => {
    const certificates = [
      {
        certificate: readFileSync(join(__dirname, 'assets/certs/test-ssl1.cer'))
          .toString('utf-8')
          .trim(),
        key: readFileSync(join(__dirname, 'assets/certs/test-ssl1.key'))
          .toString('utf-8')
          .trim(),
      },
      {
        certificate: readFileSync(join(__dirname, 'assets/certs/test-ssl2.cer'))
          .toString('utf-8')
          .trim(),
        key: readFileSync(join(__dirname, 'assets/certs/test-ssl2.key'))
          .toString('utf-8')
          .trim(),
      },
    ];
    const ssl1SNIs = ['ssl1-1.com', 'ssl1-2.com'];
    const ssl1 = {
      snis: ssl1SNIs,
      certificates: [certificates[0]],
    } as ADCSDK.SSL;
    const ssl2SNIs = ['ssl2-1.com', 'ssl2-2.com'];
    const ssl2 = {
      snis: ssl2SNIs,
      certificates: [certificates[1]],
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

      result.ssls = sortResult(result.ssls, 'id');
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
      expect(result.ssls[0]).toMatchObject(testCase);
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
