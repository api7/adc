import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPISIX } from '../../src';
import { server, token } from '../support/constants';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
} from '../support/utils';

describe('Upstream E2E', () => {
  let backend: BackendAPISIX;

  beforeAll(() => {
    backend = new BackendAPISIX({
      server,
      token,
      tlsSkipVerify: true,
    });
  });

  describe('Sync and dump upstream (nodes = null)', () => {
    const upstream = {
      scheme: 'https',
      discovery_type: 'kubernetes',
      service_name: 'test',
    } as ADCSDK.Upstream;
    const serviceName = 'service1';
    const service = {
      name: serviceName,
      upstream: structuredClone(upstream),
    } as ADCSDK.Service;

    it('Create services', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service);
    });

    it('Delete service', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));

    it('Dump again', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });
});
