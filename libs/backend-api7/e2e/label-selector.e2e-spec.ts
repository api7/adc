import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPI7 } from '../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
} from './support/utils';

describe('Label Selector', () => {
  const commonBackendOpts = {
    server: process.env.SERVER,
    token: process.env.TOKEN,
    tlsSkipVerify: true,
    gatewayGroup: 'default',
  };
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7(commonBackendOpts);
  });

  describe('Consumer', () => {
    const consumer1Name = 'consumer1';
    const consumer1 = {
      username: consumer1Name,
      labels: { team: '1' },
      plugins: {
        'key-auth': {
          key: consumer1Name,
        },
      },
    } as ADCSDK.Consumer;
    const consumer2Name = 'consumer2';
    const consumer2 = {
      username: consumer2Name,
      labels: { team: '2' },
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

    it('Dump consumer whit label team = 1', async () => {
      const backend = new BackendAPI7({
        ...commonBackendOpts,
        labelSelector: { team: '1' }, // add custom label selector
      });
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(1);
      expect(result.consumers[0]).toMatchObject(consumer1);
    });

    it('Dump consumer whit label team = 2', async () => {
      const backend = new BackendAPI7({
        ...commonBackendOpts,
        labelSelector: { team: '2' }, // add custom label selector
      });
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(1);
      expect(result.consumers[0]).toMatchObject(consumer2);
    });

    it('Delete consumers', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
        deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
      ]));
  });
});
