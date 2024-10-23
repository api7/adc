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

  conditionalDescribe(semverCondition(gte, '3.11.0'))(
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
        console.log(result.consumers[0]);
        expect(result.consumers[0].credentials).toBeUndefined();
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
