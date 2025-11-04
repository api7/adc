import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3.js';

describe('Differ V3 - service with upstreams', () => {
  it('should be considered unchanged (only upstream)', () => {
    const service = {
      id: 'service1',
      name: 'service1',
      upstream: {
        nodes: [{ host: 'upstream1', port: 80, weight: 1 }],
      },
    };
    expect(
      DifferV3.diff({ services: [service] }, { services: [service] }),
    ).toEqual([] as Array<ADCSDK.Event>);
  });

  it('should be considered unchanged (upstream + upstreams)', () => {
    const service = {
      id: 'service1',
      name: 'service1',
      upstream: { nodes: [{ host: 'upstream1', port: 80, weight: 1 }] },
      upstreams: [{ id: 'non-default', name: 'non-default' }],
    };
    expect(
      DifferV3.diff({ services: [service] }, { services: [service] }),
    ).toEqual([] as Array<ADCSDK.Event>);
  });

  it('should update upstream', () => {
    const serviceName = 'service1';
    const upstreamName = 'upstream1';
    const service = {
      id: serviceName,
      name: serviceName,
      upstream: { nodes: [{ host: upstreamName, port: 80, weight: 1 }] },
    };
    expect(
      DifferV3.diff(
        { services: [service] },
        {
          services: [
            {
              ...structuredClone(service),
              upstream: {
                name: upstreamName,
                nodes: [{ host: upstreamName, port: 80, weight: 1 }],
              },
            },
          ],
        },
      ),
    ).toMatchObject([
      {
        diff: [{ kind: 'D', lhs: upstreamName, path: ['upstream', 'name'] }],
        resourceId: serviceName,
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should create service and upstream', () => {
    const serviceName = 'service1';
    const upstream1Name = 'upstream1';
    const upstream2Name = 'upstream2';
    const service = {
      id: serviceName,
      name: serviceName,
      upstream: { nodes: [{ host: upstream1Name, port: 80, weight: 1 }] },
      upstreams: [{ id: upstream2Name, name: upstream2Name }],
    };
    expect(DifferV3.diff({ services: [service] }, {})).toEqual([
      {
        newValue: {
          name: serviceName,
          upstream: { nodes: [{ host: upstream1Name, port: 80, weight: 1 }] },
          upstreams: [{ name: upstream2Name }],
        },
        resourceId: serviceName,
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.CREATE,
      },
      {
        newValue: { name: upstream2Name },
        parentId: serviceName,
        resourceId: upstream2Name,
        resourceName: upstream2Name,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        type: ADCSDK.EventType.CREATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should create non-default upstreams', () => {
    const serviceName = 'service1';
    const upstreamName = 'upstream1';
    const service = {
      id: serviceName,
      name: serviceName,
      upstreams: [{ name: upstreamName }],
    };
    expect(
      DifferV3.diff(
        { services: [service] },
        {
          services: [
            {
              ...structuredClone(service),
              upstreams: undefined,
            },
          ],
        },
      ),
    ).toEqual([
      {
        newValue: { name: upstreamName },
        parentId: serviceName,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${upstreamName}`),
        resourceName: upstreamName,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        type: ADCSDK.EventType.CREATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should replace non-default upstreams', () => {
    const serviceName = 'service1';
    const upstream1Name = 'upstream1';
    const upstream2Name = 'upstream2';
    const service = {
      id: serviceName,
      name: serviceName,
      upstreams: [{ name: upstream1Name }],
    };
    expect(
      DifferV3.diff(
        { services: [service] },
        {
          services: [
            {
              ...structuredClone(service),
              // @ts-expect-error testing purposes
              upstreams: [{ id: upstream2Name, name: upstream2Name }],
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: { name: upstream2Name },
        parentId: serviceName,
        resourceId: upstream2Name,
        resourceName: upstream2Name,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        type: ADCSDK.EventType.DELETE, // Delete the old upstream
      },
      {
        newValue: { name: upstream1Name },
        parentId: serviceName,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${upstream1Name}`),
        resourceName: upstream1Name,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        type: ADCSDK.EventType.CREATE, // Create the new upstream
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should update non-default upstreams', () => {
    const serviceName = 'service1';
    const upstreamName = 'upstream1';
    const service = {
      id: serviceName,
      name: serviceName,
      upstreams: [
        {
          name: upstreamName,
          nodes: [{ host: upstreamName, port: 80, weight: 1 }],
        },
      ],
    };
    expect(
      DifferV3.diff(
        { services: [service] },
        {
          services: [
            {
              ...structuredClone(service),
              upstreams: [
                {
                  // @ts-expect-error testing purposes
                  id: ADCSDK.utils.generateId(`${serviceName}.${upstreamName}`),
                  name: upstreamName,
                  nodes: [{ host: '1.1.1.1', port: 80, weight: 1 }],
                },
              ],
            },
          ],
        },
      ),
    ).toEqual([
      {
        diff: [
          {
            kind: 'E',
            lhs: '1.1.1.1',
            path: ['nodes', 0, 'host'],
            rhs: upstreamName,
          },
        ],
        newValue: {
          name: upstreamName,
          nodes: [{ host: upstreamName, port: 80, weight: 1 }],
        },
        oldValue: {
          name: upstreamName,
          nodes: [{ host: '1.1.1.1', port: 80, weight: 1 }],
        },
        parentId: serviceName,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${upstreamName}`),
        resourceName: upstreamName,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        type: ADCSDK.EventType.UPDATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should delete non-default upstreams', () => {
    const serviceName = 'service1';
    const upstreamName = 'upstream1';
    const service = {
      id: serviceName,
      name: serviceName,
      upstreams: [
        {
          id: ADCSDK.utils.generateId(`${serviceName}.${upstreamName}`),
          name: upstreamName,
        },
      ],
    };
    expect(
      DifferV3.diff(
        {
          services: [
            {
              ...structuredClone(service),
              upstreams: undefined,
            },
          ],
        },
        { services: [service] },
      ),
    ).toEqual([
      {
        oldValue: { name: upstreamName },
        parentId: serviceName,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${upstreamName}`),
        resourceName: upstreamName,
        resourceType: ADCSDK.ResourceType.UPSTREAM,
        type: ADCSDK.EventType.DELETE,
      },
    ] as Array<ADCSDK.Event>);
  });
});
