import * as ADCSDK from '@api7/adc-sdk';
import { unset } from 'lodash';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

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
        deleteEvent(ADCSDK.ResourceType.CONSUMER, consumer2Name),
      ]));
  });

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
