import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPI7 } from '../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  overrideEventResourceId,
  syncEvents,
} from './support/utils';

describe('Miscellaneous', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  describe('Sync resources with the name/description greater than 256 bytes', () => {
    const routeName = ''.padEnd(64 * 1024, '0'); // 65536 bytes
    const serviceName = ''.padEnd(64 * 1024, '0'); // 65536 bytes
    const route = {
      name: routeName,
      uris: ['/test'],
    };
    const service = {
      name: serviceName,
      description: ''.padEnd(64 * 1024, '0'), // 65536 bytes
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
      routes: [route],
    } as ADCSDK.Service;

    it('Create services', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
        createEvent(ADCSDK.ResourceType.ROUTE, routeName, route, serviceName),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].routes[0]).toMatchObject(route);
    });

    it('Delete service', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.ROUTE, routeName, serviceName),
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });

  describe('Sync resources with custom id', () => {
    const routeName = 'Test Route';
    const serviceName = 'Test Service';
    const route = {
      id: 'custom-route',
      name: routeName,
      uris: ['/test'],
    };
    const service = {
      id: 'custom-service',
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
      routes: [route],
    } as ADCSDK.Service;

    it('Create services', async () =>
      syncEvents(backend, [
        overrideEventResourceId(
          createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
          'custom-service',
        ),
        overrideEventResourceId(
          createEvent(ADCSDK.ResourceType.ROUTE, routeName, route, serviceName),
          'custom-route',
          'custom-service',
        ),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].routes[0]).toMatchObject(route);
    });

    it('Delete service', async () =>
      syncEvents(backend, [
        overrideEventResourceId(
          deleteEvent(ADCSDK.ResourceType.ROUTE, routeName, serviceName),
          'custom-route',
          'custom-service',
        ),
        overrideEventResourceId(
          deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
          'custom-service',
        ),
      ]));

    it('Dump again', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });
});
