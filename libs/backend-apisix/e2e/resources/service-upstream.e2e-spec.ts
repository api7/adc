import * as ADCSDK from '@api7/adc-sdk';
import { gte } from 'semver';

import { BackendAPISIX } from '../../src';
import { server, token } from '../support/constants';
import {
  conditionalDescribe,
  createEvent,
  deleteEvent,
  dumpConfiguration,
  semverCondition,
  sortResult,
  syncEvents,
  updateEvent,
} from '../support/utils';

conditionalDescribe(semverCondition(gte, '3.5.0'))(
  'Service-Upstreams E2E',
  () => {
    let backend: BackendAPISIX;

    beforeAll(() => {
      backend = new BackendAPISIX({
        server,
        token,
        tlsSkipVerify: true,
      });
    });

    describe('Service multiple upstreams', () => {
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
      } satisfies ADCSDK.Service;
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

      it('Create service and upstreams', async () =>
        syncEvents(backend, [
          createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
          createEvent(
            ADCSDK.ResourceType.UPSTREAM,
            upstreamND1Name,
            upstreamND1,
            serviceName,
          ),
          createEvent(
            ADCSDK.ResourceType.UPSTREAM,
            upstreamND2Name,
            upstreamND2,
            serviceName,
          ),
        ]));

      it('Dump', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(1);
        expect(result.services[0]).toMatchObject(service);
        expect(result.services[0].upstreams).toHaveLength(2);

        const upstreams = sortResult(result.services[0].upstreams, 'name');
        expect(upstreams[0]).toMatchObject(upstreamND1);
        expect(upstreams[1]).toMatchObject(upstreamND2);
      });

      const newUpstreamND1 = {
        ...structuredClone(upstreamND1),
        retry_timeout: 100,
      } as ADCSDK.Upstream;
      it('Update service non-default upstream 1', async () =>
        syncEvents(backend, [
          updateEvent(
            ADCSDK.ResourceType.UPSTREAM,
            upstreamND1Name,
            newUpstreamND1,
            serviceName,
          ),
        ]));

      it('Dump (updated non-default upstream 1)', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(1);

        const upstreams = sortResult(result.services[0].upstreams, 'name');
        expect(upstreams[0]).toMatchObject(newUpstreamND1);
      });

      it('Delete non-default upstream 2', async () =>
        syncEvents(backend, [
          deleteEvent(
            ADCSDK.ResourceType.UPSTREAM,
            upstreamND2Name,
            serviceName,
          ),
        ]));

      it('Dump (non-default upstream 2 should not exist)', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(1);
        expect(result.services[0].upstreams).toHaveLength(1);
        expect(result.services[0].upstreams[0]).toMatchObject(newUpstreamND1);
      });

      it('Delete', async () =>
        syncEvents(backend, [
          deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
        ]));

      it('Dump again (service should not exist)', async () => {
        const result = await dumpConfiguration(backend);
        expect(result.services).toHaveLength(0);
      });
    });
  },
);
