import * as ADCSDK from '@api7/adc-sdk';
import { gte, lt } from 'semver';

import { BackendAPI7 } from '../../src';
import { conditionalDescribe, semverCondition } from '../support/utils';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  syncEvents,
  updateEvent,
} from '../support/utils';

describe('Consumer E2E', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  conditionalDescribe(semverCondition(lt, '3.2.15'))(
    'Sync and dump consumers (without credential support)',
    () => {
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
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(2);
        expect(result.consumers[0]).toMatchObject(consumer2);
        expect(result.consumers[1]).toMatchObject(consumer1);
      });

      it('Update consumer1', async () => {
        consumer1.description = 'desc';
        await syncEvents(backend, [
          updateEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name, consumer1),
        ]);
      });

      it('Dump again (consumer1 updated)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers[0]).toMatchObject(consumer1);
      });

      it('Delete consumer1', async () =>
        syncEvents(backend, [
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
        ]));

      it('Dump again (consumer1 should not exist)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer2);
      });

      it('Delete consumer2', async () =>
        syncEvents(backend, [
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name),
        ]));

      it('Dump again (consumer2 should not exist)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(0);
      });
    },
  );

  conditionalDescribe(semverCondition(gte, '3.2.15'))(
    'Sync and dump consumers (with credential support)',
    () => {
      const consumer1Name = 'consumer1';
      const consumer1Key = 'consumer1-key';
      const consumer1Cred = {
        name: consumer1Key,
        type: 'key-auth',
        config: { key: consumer1Key },
      };
      const consumer1 = {
        username: consumer1Name,
        credentials: [consumer1Cred],
      } as ADCSDK.Consumer;

      it('Create consumers', async () =>
        syncEvents(backend, [
          createEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name, consumer1),
          createEvent(
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
            consumer1Key,
            consumer1Cred,
            consumer1Name,
          ),
        ]));

      it('Dump', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer1);
        expect(result.consumers[0].credentials).toMatchObject(
          consumer1.credentials,
        );
      });

      it('Update consumer credential', async () => {
        consumer1.credentials[0].config.key = 'new-key';
        await syncEvents(backend, [
          updateEvent(
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
            consumer1Key,
            consumer1Cred,
            consumer1Name,
          ),
        ]);
      });

      it('Dump again (consumer credential updated)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers[0]).toMatchObject(consumer1);
        expect(result.consumers[0].credentials[0].config.key).toEqual(
          'new-key',
        );
      });

      it('Delete consumer credential', async () =>
        syncEvents(backend, [
          deleteEvent(
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
            consumer1Key,
            consumer1Name,
          ),
        ]));

      it('Dump again (consumer credential should not exist)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer1);
        expect(result.consumers[0].credentials).toHaveLength(0);
      });

      it('Delete consumer', async () =>
        syncEvents(backend, [
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
        ]));

      it('Dump again (consumer should not exist)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(0);
      });
    },
  );
});
