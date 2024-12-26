import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPI7 } from '../../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
} from '../support/utils';

describe('Route E2E', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  describe('Timeout', () => {
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
      uris: ['/route1'],
      timeout: {
        connect: 111,
        send: 222,
        read: 333,
      },
    } as ADCSDK.Route;

    it('Create resources', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
        createEvent(ADCSDK.ResourceType.ROUTE, route1Name, route1, serviceName),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].routes).toHaveLength(1);
      expect(result.services[0].routes[0]).toMatchObject(route1);
    });

    it('Delete', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again (service should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });

  describe('Vars', () => {
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
      uris: ['/route1'],
      vars: [['remote_addr', '==', '1.1.1.1']],
    } as ADCSDK.Route;

    it('Create resources', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
        createEvent(ADCSDK.ResourceType.ROUTE, route1Name, route1, serviceName),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
      expect(result.services[0].routes).toHaveLength(1);
      expect(result.services[0].routes[0]).toMatchObject(route1);
    });

    it('Delete', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again (service should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });
});
