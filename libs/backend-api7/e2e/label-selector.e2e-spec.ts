import * as ADCSDK from '@api7/adc-sdk';
import { create, unset } from 'lodash';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { gte, lt, lte } from 'semver';

import { BackendAPI7 } from '../src';
import {
  conditionalDescribe,
  createEvent,
  deleteEvent,
  dumpConfiguration,
  semverCondition,
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

  conditionalDescribe(semverCondition(lt, '3.2.15'))(
    'Consumer (without credential support)',
    () => {
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
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer1);
      });

      it('Dump consumer whit label team = 2', async () => {
        const backend = new BackendAPI7({
          ...commonBackendOpts,
          labelSelector: { team: '2' }, // add custom label selector
        });
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer2);
      });

      it('Delete consumers', async () =>
        syncEvents(backend, [
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name),
        ]));
    },
  );

  conditionalDescribe(semverCondition(gte, '3.2.15'))(
    'Consumer (with credential support)',
    () => {
      const credential1 = {
        name: 'key-1',
        labels: { team: '1' },
        type: 'key-auth',
        config: {
          key: 'key-1',
        },
      };
      const credential2 = {
        name: 'key-2',
        labels: { team: '2' },
        type: 'key-auth',
        config: {
          key: 'key-2',
        },
      };
      const consumer1Name = 'consumer1';
      const consumer1 = {
        username: consumer1Name,
        labels: { team: '1' },
      } as ADCSDK.Consumer;
      const consumer2Name = 'consumer2';
      const consumer2 = {
        username: consumer2Name,
        labels: { team: '2' },
      } as ADCSDK.Consumer;

      it('Create consumers', async () =>
        syncEvents(backend, [
          createEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name, consumer1),
          createEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name, consumer2),
          createEvent(
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
            credential1.name,
            credential1,
            consumer1Name,
          ),
          createEvent(
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
            credential2.name,
            credential2,
            consumer2Name,
          ),
        ]));

      it('Dump consumer whit label team = 1', async () => {
        const backend = new BackendAPI7({
          ...commonBackendOpts,
          labelSelector: { team: '1' }, // add custom label selector
        });
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer1);
        expect(result.consumers[0].credentials).toHaveLength(2);
      });

      it('Dump consumer whit label team = 2', async () => {
        const backend = new BackendAPI7({
          ...commonBackendOpts,
          labelSelector: { team: '2' }, // add custom label selector
        });
        const result = (await dumpConfiguration(
          backend,
        )) as ADCSDK.Configuration;
        expect(result.consumers).toHaveLength(1);
        expect(result.consumers[0]).toMatchObject(consumer2);
      });

      it('Delete consumers', async () =>
        syncEvents(backend, [
          deleteEvent(
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
            credential1.name,
            consumer1Name,
          ),
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer1Name),
          deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name),
        ]));
    },
  );

  describe('SSL', () => {
    const certificates = [
      {
        certificate: readFileSync(
          join(__dirname, 'assets/certs/test-ssl1.cer'),
        ).toString('utf-8'),
        key: readFileSync(
          join(__dirname, 'assets/certs/test-ssl1.key'),
        ).toString('utf-8'),
      },
      {
        certificate: readFileSync(
          join(__dirname, 'assets/certs/test-ssl2.cer'),
        ).toString('utf-8'),
        key: readFileSync(
          join(__dirname, 'assets/certs/test-ssl2.key'),
        ).toString('utf-8'),
      },
    ];
    const ssl1SNIs = ['ssl1-1.com', 'ssl1-2.com'];
    const ssl1 = {
      snis: ssl1SNIs,
      labels: { team: '1' },
      certificates: [certificates[0]],
    } as ADCSDK.SSL;
    const ssl2SNIs = ['ssl2-1.com', 'ssl2-2.com'];
    const ssl2 = {
      snis: ssl2SNIs,
      labels: { team: '2' },
      certificates: [certificates[1]],
    } as ADCSDK.SSL;
    const sslName = (snis: Array<string>) => snis.join(',');

    const ssl1test = structuredClone(ssl1);
    const ssl2test = structuredClone(ssl2);
    unset(ssl1test, 'certificates.0.key');
    unset(ssl2test, 'certificates.0.key');

    it('Create ssls', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SSL, sslName(ssl1SNIs), ssl1),
        createEvent(ADCSDK.ResourceType.SSL, sslName(ssl2SNIs), ssl2),
      ]));

    it('Dump consumer whit label team = 1', async () => {
      const backend = new BackendAPI7({
        ...commonBackendOpts,
        labelSelector: { team: '1' }, // add custom label selector
      });
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.ssls).toHaveLength(1);
      expect(result.ssls[0]).toMatchObject(ssl1test);
    });

    it('Dump consumer whit label team = 2', async () => {
      const backend = new BackendAPI7({
        ...commonBackendOpts,
        labelSelector: { team: '2' }, // add custom label selector
      });
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.ssls).toHaveLength(1);
      expect(result.ssls[0]).toMatchObject(ssl2test);
    });

    it('Delete ssls', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SSL, sslName(ssl1SNIs)),
        deleteEvent(ADCSDK.ResourceType.SSL, sslName(ssl2SNIs)),
      ]));
  });
});
