import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPISIX } from '../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  overrideEventResourceId,
  syncEvents,
} from './support/utils';

describe('Miscellaneous', () => {
  let backend: BackendAPISIX;

  beforeAll(() => {
    backend = new BackendAPISIX({
      server: process.env.SERVER,
      token: process.env.TOKEN,
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
