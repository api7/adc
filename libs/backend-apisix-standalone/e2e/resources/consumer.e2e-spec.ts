import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';

import { BackendAPISIXStandalone } from '../../src';
import * as typing from '../../src/typing';
import { server1, token1 } from '../support/constants';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  restartAPISIX,
  syncEvents,
  updateEvent,
} from '../support/utils';

describe('Consumer E2E', () => {
  let backend: BackendAPISIXStandalone;

  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server: server1,
      token: token1,
      tlsSkipVerify: true,
      cacheKey: 'default',
    });
  });

  describe('Sync and dump consumers (with credential support)', () => {
    const consumer1Name = 'consumer1';
    const consumer1Key = 'consumer1-key';
    const consumer1Cred = {
      name: consumer1Key,
      type: 'key-auth',
      config: { key: consumer1Key },
    };
    const consumer1Key2 = 'consumer1-key2';
    const consumer1Cred2 = {
      name: consumer1Key2,
      type: 'key-auth',
      config: { key: consumer1Key2 },
    };
    const consumer1 = {
      username: consumer1Name,
      credentials: [consumer1Cred, consumer1Cred2],
    } as ADCSDK.Consumer;

    it('Initialize cache', () =>
      expect(dumpConfiguration(backend)).resolves.not.toThrow());

    it('Create consumers', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name, consumer1),
        createEvent(
          ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
          consumer1Key,
          consumer1Cred,
          consumer1Name,
        ),
        createEvent(
          ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
          consumer1Key2,
          consumer1Cred2,
          consumer1Name,
        ),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(1);
      expect(result.consumers![0]).toMatchObject(consumer1);
      expect(result.consumers![0].credentials).toHaveLength(2);
      expect(result.consumers![0].credentials).toMatchObject(
        consumer1.credentials!,
      );
    });

    it('Update consumer credential1', async () => {
      const newCred = structuredClone(consumer1Cred);
      newCred.config.key = 'new-key';
      await syncEvents(backend, [
        updateEvent(
          ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
          consumer1Key,
          newCred,
          consumer1Name,
        ),
      ]);
    });

    it('Dump again (consumer credential1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers![0].credentials![0].config.key).toEqual(
        'new-key',
      );

      // Access Admin API to check modifiedIndex and conf_version
      const resp = await axios.get(`${server1}/apisix/admin/configs`, {
        headers: {
          'X-API-KEY': token1,
        },
      });
      expect(resp.data.consumers_conf_version).toBeGreaterThan(1);
      resp.data.consumers
        ?.filter(
          (item: typing.Consumer | typing.ConsumerCredential) =>
            'name' in item && item.name === 'consumer1-key',
        )
        .forEach((item: typing.ConsumerCredential) =>
          expect(item.modifiedIndex).toBeGreaterThan(1),
        );
    });

    it('Delete consumer credential1', async () =>
      syncEvents(backend, [
        deleteEvent(
          ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
          consumer1Key,
          consumer1Name,
        ),
      ]));

    it('Dump again (consumer credential should only keep credential2)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(1);
      expect(result.consumers![0].credentials).toHaveLength(1);
      expect(result.consumers![0].credentials![0]).toMatchObject(
        consumer1Cred2,
      );
    });

    it('Delete consumer', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
        deleteEvent(
          ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
          consumer1Key2,
          consumer1Name,
        ),
      ]));

    it('Dump again (consumer should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.consumers).toHaveLength(0);
    });
  });
});
