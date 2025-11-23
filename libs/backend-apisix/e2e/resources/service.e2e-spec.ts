import { Differ } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';

import { BackendAPISIX } from '../../src';
import * as typing from '../../src/typing';
import { defaultBackendOptions, server, token } from '../support/constants';
import { cleanup, syncEvents } from '../support/utils';

describe('Service E2E', () => {
  let backend: BackendAPISIX;

  beforeAll(async () => {
    backend = new BackendAPISIX(defaultBackendOptions);

    await cleanup(backend);
  });

  afterAll(async () => {
    await cleanup(backend);
  });

  describe('should split inline upstream', () => {
    const serviceName = 'test';
    const service: ADCSDK.Service = {
      id: serviceName,
      name: serviceName,
      upstream: {
        type: 'roundrobin',
        nodes: [
          {
            host: '127.0.0.1',
            port: 8080,
            weight: 1,
          },
        ],
      },
    };

    it('create service', async () =>
      syncEvents(backend, Differ.diff({ services: [service] }, {})));

    it('get service', async () => {
      const data = (await fetch(
        `${server}/apisix/admin/services/${serviceName}`,
        { headers: { 'X-API-KEY': token } },
      ).then((res) => res.json())) as { value: typing.Service };

      expect(data).toBeDefined();
      expect(data.value.upstream_id).toBeDefined();
      expect(data.value.upstream).toBeUndefined();
    });

    it('cleanup service', async () =>
      syncEvents(backend, Differ.diff({}, { services: [service] })));
  });
});
