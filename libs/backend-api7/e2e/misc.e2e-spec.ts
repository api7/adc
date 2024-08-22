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
  updateEvent,
} from './support/utils';

describe('Miscellaneous', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  describe('Sync resources with the description greater than 256 bytes', () => {
    const service1Name = 'service1';
    const service1 = {
      name: service1Name,
      description: ''.padEnd(64 * 1024, '0'), // 65536 bytes
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
    } as ADCSDK.Service;

    it('Create services', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(1);
      expect(result.services[0]).toMatchObject(service1);
    });

    it('Delete service', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.SERVICE, service1Name),
      ]));

    it('Dump again', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.services).toHaveLength(0);
    });
  });
});
