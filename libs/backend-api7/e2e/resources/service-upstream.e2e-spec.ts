import { Differ } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { unset } from 'lodash';
import { gte } from 'semver';

import { BackendAPI7 } from '../../src';
import {
  conditionalDescribe,
  dumpConfiguration,
  semverCondition,
  sortResult,
  syncEvents,
} from '../support/utils';

conditionalDescribe(semverCondition(gte, '3.5.0'))(
  'Service-Upstreams E2E',
  () => {
    let backend: BackendAPI7;

    beforeAll(() => {
      backend = new BackendAPI7({
        server: process.env.SERVER!,
        token: process.env.TOKEN!,
        tlsSkipVerify: true,
        gatewayGroup: process.env.GATEWAY_GROUP,
        cacheKey: 'e2e-service-upstream',
      });
    });

    describe('Service multiple upstreams', () => {
      const upstreamND1Name = 'nd-upstream1';
      const upstreamND1 = {
        name: upstreamND1Name,
        type: 'roundrobin',
        scheme: 'https',
        nodes: [
          {
            host: '1.1.1.1',
            port: 443,
            weight: 100,
          },
        ],
      } satisfies ADCSDK.Upstream;
      const upstreamND2Name = 'nd-upstream2';
      const upstreamND2 = {
        name: upstreamND2Name,
        type: 'roundrobin',
        scheme: 'https',
        nodes: [
          {
            host: '1.0.0.1',
            port: 443,
            weight: 100,
          },
        ],
      } satisfies ADCSDK.Upstream;
      const serviceName = 'test';
      const service = {
        name: serviceName,
        upstream: {
          type: 'roundrobin',
          nodes: [
            {
              host: 'httpbin.org',
              port: 443,
              weight: 100,
            },
          ],
        },
        path_prefix: '/test',
        strip_path_prefix: true,
        upstreams: [upstreamND1, upstreamND2],
      } satisfies ADCSDK.Service;

      it('Create service and upstreams', async () =>
        syncEvents(
          backend,
          Differ.diff(
            { services: [service] },
            await dumpConfiguration(backend),
          ),
        ));

      it('Dump', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(1);
        const testService = service;
        unset(testService, 'upstreams');
        expect(result.services![0]).toMatchObject(testService);
        expect(result.services![0].upstreams).toHaveLength(2);

        const upstreams = sortResult(result.services![0].upstreams!, 'name');
        expect(upstreams[0]).toMatchObject(upstreamND1);
        expect(upstreams[1]).toMatchObject(upstreamND2);
      });

      const newUpstreamND1 = {
        ...structuredClone(upstreamND1),
        retry_timeout: 100,
      } as ADCSDK.Upstream;
      it('Update service non-default upstream 1', async () =>
        syncEvents(
          backend,
          Differ.diff(
            {
              services: [
                {
                  ...service,
                  upstreams: [newUpstreamND1, upstreamND2],
                },
              ],
            },
            await dumpConfiguration(backend),
          ),
        ));

      it('Dump (updated non-default upstream 1)', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(1);

        const upstreams = sortResult(result.services![0].upstreams!, 'name');
        expect(upstreams[0]).toMatchObject(newUpstreamND1);
      });

      it('Delete non-default upstream 2', async () => {
        await syncEvents(
          backend,
          Differ.diff(
            {
              services: [
                {
                  ...service,
                  upstreams: [newUpstreamND1],
                },
              ],
            },
            await dumpConfiguration(backend),
          ),
        );
      });

      it('Dump (non-default upstream 2 should not exist)', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(1);
        expect(result.services![0].upstreams).toHaveLength(1);
        expect(result.services![0].upstreams![0]).toMatchObject(newUpstreamND1);
      });

      it('Delete', async () =>
        syncEvents(backend, Differ.diff({}, await dumpConfiguration(backend))));

      it('Dump again (service should not exist)', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(0);
      });
    });
  },
);
