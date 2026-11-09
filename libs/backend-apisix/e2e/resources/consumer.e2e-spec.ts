import { Differ } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { gte } from 'semver';

import { BackendAPISIX } from '../../src';
import { defaultBackendOptions } from '../support/constants';
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
    backend = new BackendAPISIX(defaultBackendOptions);
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
        expect(result.consumers![0]).toMatchObject(consumer1);
        expect(result.consumers![0].credentials).toMatchObject(
          consumer1.credentials!,
        );
      });

      it('Update consumer credential', async () => {
        consumer1.credentials![0].config.key = 'new-key';
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
        expect(result.consumers![0]).toMatchObject(consumer1);
        expect(result.consumers![0].credentials![0].config.key).toEqual(
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
        expect(result.consumers![0].credentials).toBeUndefined();
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

  conditionalDescribe(semverCondition(gte, '3.11.0'))(
    'Sync consumer credential deletes produced by the differ',
    () => {
      const credential = (name: string) => ({
        name,
        type: 'key-auth',
        config: { key: name },
      });
      const syncDiff = async (local: ADCSDK.Configuration) =>
        syncEvents(
          backend,
          Differ.diff(local, await dumpConfiguration(backend)),
        );

      it('Create consumers with credentials', async () =>
        syncDiff({
          consumers: [
            {
              username: 'cascade-gone',
              credentials: [credential('gone-1'), credential('gone-2')],
            },
            {
              username: 'cascade-kept',
              credentials: [credential('kept-1'), credential('kept-2')],
            },
          ],
        }));

      // The differ emits a deleted consumer's credential deletes after the
      // consumer delete; APISIX already removed them with the consumer.
      it('Delete a consumer and rename a kept consumer credential', async () =>
        syncDiff({
          consumers: [
            {
              username: 'cascade-kept',
              credentials: [credential('kept-1'), credential('kept-3')],
            },
          ],
        }));

      it('Dump (deleted consumer gone, kept consumer updated)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers?.[0]?.username).toEqual('cascade-kept');
        expect(
          result.consumers?.[0]?.credentials?.map(({ name }) => name).sort(),
        ).toEqual(['kept-1', 'kept-3']);
      });

      it('Delete all consumers', async () => syncDiff({ consumers: [] }));

      it('Dump again (no consumers)', async () => {
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(0);
      });
    },
  );
});
