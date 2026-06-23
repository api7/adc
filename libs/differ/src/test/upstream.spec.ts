import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3.js';
import { DifferV4 } from '../differv4.js';

describe.each([
  ['V3', DifferV3],
  ['V4', DifferV4],
] as const)('Differ %s - upstream', (_version, Differ) => {
  it('should create and update ssl before upstream', () => {
    const serviceName = 'test';
    const upstreamName = 'upstream-with-client-cert';
    expect(
      Differ.diff(
        {
          services: [
            {
              id: serviceName,
              name: serviceName,
              routes: [],
              upstream: {
                type: 'roundrobin',
                nodes: [{ host: '127.0.0.1', port: 80, weight: 1 }],
              },
              upstreams: [
                // We need to use multiple upstreams to test this scenario.
                // The key reason is that upstream splitting within the service occurs at each backend rather than within this diff.
                // This necessitates ensuring SSL creates and updates are always completed before the upstream and service.
                // We can precisely test this through the multiple upstreams functionality.
                {
                  name: upstreamName,
                  type: 'roundrobin',
                  nodes: [{ host: '127.0.0.1', port: 80, weight: 1 }],
                  tls: {
                    client_cert_id: 'test',
                  },
                },
              ],
            },
          ],
          ssls: [
            {
              id: 'test',
              snis: ['test1.com', 'test2.com'],
              certificates: [
                {
                  certificate:
                    '-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----',
                  key: '-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----',
                },
              ],
            },
            {
              id: 'test2',
              snis: ['test3.com', 'test4.com'],
              certificates: [
                {
                  certificate:
                    '-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----',
                  key: '-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----',
                },
              ],
            },
          ],
        },
        {
          services: [
            {
              id: 'test',
              name: 'test',
              routes: [],
              upstream: {
                type: 'roundrobin',
                nodes: [{ host: '127.0.0.1', port: 80, weight: 1 }],
              },
            },
          ],
          ssls: [
            {
              id: 'test2',
              snis: ['test3.com', 'test4.com', 'test5.com'],
              certificates: [
                {
                  certificate:
                    '-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----',
                  key: '-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----',
                },
              ],
            },
          ],
        },
      ),
    ).toMatchObject([
      {
        type: ADCSDK.EventType.CREATE,
        resourceType: ADCSDK.ResourceType.SSL,
        resourceId: 'test',
      },
      {
        type: ADCSDK.EventType.UPDATE,
        resourceType: ADCSDK.ResourceType.SSL,
        resourceId: 'test2',
      },
      {
        type: ADCSDK.EventType.CREATE,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${upstreamName}`),
      },
    ] as Array<ADCSDK.Event>);
  });
});
