import * as ADCSDK from '@api7/adc-sdk';
import { gte, lt } from 'semver';

import { BackendAPISIX } from '../../src';
import { server, token } from '../support/constants';
import { conditionalDescribe, semverCondition } from '../support/utils';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
  updateEvent,
} from '../support/utils';

describe('Consumer E2E', () => {
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
});
