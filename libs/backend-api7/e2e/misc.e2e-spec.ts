import * as ADCSDK from '@api7/adc-sdk';
import { toString } from 'lodash';

import { BackendAPI7 } from '../src';
import { createEvent, syncEvents } from './support/utils';

describe('Miscellaneous', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: process.env.GATEWAY_GROUP,
    });
  });

  describe('Sync options (exitOnFailure)', () => {
    const upstream = {
      scheme: 'https',
      nodes: [
        {
          host: 'httpbin.org',
          port: 443,
          weight: 100,
        },
      ],
    } as ADCSDK.Upstream;
    const service1Name = 'service1';
    const service1 = {
      name: service1Name,
      upstream: structuredClone(upstream),
    } as ADCSDK.Service;
    const service2Name = 'service2';
    //@ts-expect-error error test
    const service2 = {
      name: service2Name,
      path_prefix: 12345,
      upstream: structuredClone(upstream),
    } as ADCSDK.Service;

    const errorPattern = `validate request failed: request body has an error: doesn't match schema: doesn't match schema due to: Error at "/path_prefix": value must be a string`;
    it('Create services (exitOnFailure = true, default)', async () => {
      await expect(
        syncEvents(backend, [
          createEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
          createEvent(ADCSDK.ResourceType.SERVICE, service2Name, service2),
        ]),
      ).rejects.toThrow(new RegExp(errorPattern));
    });

    it('Create services (exitOnFailure = false)', async () => {
      const results = await syncEvents(
        backend,
        [
          createEvent(ADCSDK.ResourceType.SERVICE, service1Name, service1),
          createEvent(ADCSDK.ResourceType.SERVICE, service2Name, service2),
        ],
        { exitOnFailure: false },
      );

      expect(results.filter((r) => r.success)).toHaveLength(1);
      expect(toString(results.filter((r) => !r.success)[0].error)).toContain(
        errorPattern,
      );
    });
  });
});
