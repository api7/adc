import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPISIXStandalone } from '../../src';
import { server1, token1 } from '../support/constants';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
  updateEvent,
} from '../support/utils';

describe('Sync and Dump - 1', () => {
  let backend: BackendAPISIXStandalone;

  beforeAll(() => {
    backend = new BackendAPISIXStandalone({
      server: server1,
      token: token1,
      cacheKey: 'default',
    });
  });

  describe('Sync and dump empty service', () => {
    const upstream = {
      description: 'test upstream',
      scheme: 'https',
      nodes: [
        {
          host: 'httpbin.org',
          port: 443,
          weight: 100,
        },
      ],
    } as ADCSDK.Upstream;
    const service1Name = 'service1';
    const service1 = {
      name: service1Name,
      upstream: structuredClone(upstream),
      hosts: ['example1.com', 'example2.com'],
    } as ADCSDK.Service;
    const service2Name = 'service2';
    const service2 = {
      name: service2Name,
      upstream: structuredClone(upstream),
    } as ADCSDK.Service;

    it('Initialize cache', () =>
      expect(dumpConfiguration(backend)).resolves.not.toThrow());

    it('Create services', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
        createEvent(ADCSDK.ResourceType.SERVICE, service2Name, service2),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
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
      expect(result.services[1]).toMatchObject(service2);
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
});
