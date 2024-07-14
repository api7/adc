import * as ADCSDK from '@api7/adc-sdk';
import { unset } from 'lodash';

import { DifferV3 } from '../differv3';

describe('Differ V3 - basic', () => {
  it('should output empty when input is empty', () => {
    expect(DifferV3.diff({}, {})).toEqual([]);
  });

  it('should create resource', () => {
    const consumerName = 'alice';
    expect(
      DifferV3.diff(
        {
          consumers: [
            {
              username: consumerName,
              plugins: {},
            },
          ],
        },
        {},
      ),
    ).toEqual([
      {
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.CREATE,
        resourceId: consumerName,
        resourceName: consumerName,
        newValue: { username: consumerName, plugins: {} },
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should update resource', () => {
    const consumerName = 'alice';
    const consumerKey = 'alice-key';
    expect(
      DifferV3.diff(
        {
          consumers: [
            {
              username: consumerName,
              plugins: {
                'key-auth': { key: consumerKey },
              },
            },
          ],
        },
        {
          consumers: [
            {
              username: consumerName,
              plugins: {},
            },
          ],
        },
      ),
    ).toEqual([
      {
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.UPDATE,
        resourceId: consumerName,
        resourceName: consumerName,
        oldValue: { username: consumerName, plugins: {} },
        newValue: {
          username: consumerName,
          plugins: {
            'key-auth': { key: consumerKey },
          },
        },
        diff: [
          {
            kind: 'N',
            rhs: { key: consumerKey },
            path: ['plugins', 'key-auth'],
          },
        ],
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should delete resource', () => {
    const consumerName = 'alice';
    expect(
      DifferV3.diff(
        {},
        {
          consumers: [
            {
              username: consumerName,
              plugins: {},
            },
          ],
        },
      ),
    ).toEqual([
      {
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.DELETE,
        resourceId: consumerName,
        resourceName: consumerName,
        oldValue: { username: consumerName, plugins: {} },
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should be sorted by event type', () => {
    const createConsumer = 'createConsumer';
    const updatedConsumer = 'updatedConsumer';
    const deletedConsumer = 'deletedConsumer';
    expect(
      DifferV3.diff(
        {
          consumers: [
            {
              username: createConsumer,
              plugins: {},
            },
            {
              username: updatedConsumer,
              plugins: {
                'key-auth': {},
              },
            },
          ],
        },
        {
          consumers: [
            {
              username: updatedConsumer,
              plugins: {},
            },
            {
              username: deletedConsumer,
              plugins: {},
            },
          ],
        },
      ),
    ).toEqual([
      // DELETE > UPDATE > CREATE
      {
        oldValue: { plugins: {}, username: deletedConsumer },
        resourceId: deletedConsumer,
        resourceName: deletedConsumer,
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.DELETE,
      },
      {
        diff: [{ kind: 'N', path: ['plugins', 'key-auth'], rhs: {} }],
        newValue: { plugins: { 'key-auth': {} }, username: updatedConsumer },
        oldValue: { plugins: {}, username: updatedConsumer },
        resourceId: updatedConsumer,
        resourceName: updatedConsumer,
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.UPDATE,
      },
      {
        newValue: { plugins: {}, username: createConsumer },
        resourceId: createConsumer,
        resourceName: createConsumer,
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.CREATE,
      },
    ]);
  });

  it('should adapt to default values added on the backend, core', () => {
    const consumerName = 'alice';
    expect(
      DifferV3.diff(
        {
          consumers: [{ username: consumerName, plugins: {} }],
        },
        {
          consumers: [{ username: consumerName, description: '', plugins: {} }],
        },
        {
          core: {
            [ADCSDK.ResourceType.CONSUMER]: { description: '' },
          },
        },
      ),
    ).toEqual([]);
  });

  it('should adapt to default values added on the backend, plugin', () => {
    const consumerName = 'alice';
    expect(
      DifferV3.diff(
        {
          consumers: [
            { username: consumerName, plugins: { 'key-auth': { key: 'key' } } },
          ],
        },
        {
          consumers: [
            {
              username: consumerName,
              plugins: { 'key-auth': { key: 'key', added: 'added' } },
            },
          ],
        },
        {
          plugins: {
            'key-auth': { added: 'added' },
          },
        },
      ),
    ).toEqual([]);
  });

  it('should update resource, add plugin', () => {
    const consumerName = 'alice';
    const consumerKey = 'alice-key';
    expect(
      DifferV3.diff(
        {
          consumers: [
            {
              username: consumerName,
              plugins: {
                'key-auth': { key: consumerKey },
              },
            },
          ],
        },
        {
          consumers: [
            {
              username: consumerName,
              plugins: {},
            },
          ],
        },
      ),
    ).toEqual([
      {
        diff: [
          {
            kind: 'N',
            path: ['plugins', 'key-auth'],
            rhs: { key: consumerKey },
          },
        ],
        newValue: {
          plugins: { 'key-auth': { key: consumerKey } },
          username: consumerName,
        },
        oldValue: { plugins: {}, username: consumerName },
        resourceId: consumerName,
        resourceName: consumerName,
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.UPDATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should update resource, update plugin with default value', () => {
    const consumerName = 'alice';
    const oldKey = 'old-key';
    const newKey = 'new-key';
    expect(
      DifferV3.diff(
        {
          consumers: [
            {
              username: consumerName,
              plugins: {
                'key-auth': { key: newKey },
              },
            },
          ],
        },
        {
          consumers: [
            {
              username: consumerName,
              plugins: {
                'key-auth': { key: oldKey, added: 'added' },
              },
            },
          ],
        },
        {
          plugins: {
            'key-auth': { added: 'added' },
          },
        },
      ),
    ).toEqual([
      {
        diff: [
          {
            kind: 'E',
            lhs: oldKey,
            path: ['plugins', 'key-auth', 'key'],
            rhs: newKey,
          },
        ],
        newValue: {
          plugins: { 'key-auth': { key: newKey, added: 'added' } },
          username: consumerName,
        },
        oldValue: {
          plugins: { 'key-auth': { added: 'added', key: oldKey } },
          username: consumerName,
        },
        resourceId: consumerName,
        resourceName: consumerName,
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.UPDATE,
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should generate hashed resource id', () => {
    const sslName = 'demo-sni1,demo-sni2';
    const ssl = {
      snis: ['demo-sni1', 'demo-sni2'],
      certificates: [{ certificate: 'cert', key: 'key' }],
    };
    expect(
      DifferV3.diff(
        {
          ssls: [ssl],
        },
        {},
      ),
    ).toEqual([
      {
        newValue: ssl,
        resourceId: ADCSDK.utils.generateId(sslName),
        resourceName: sslName,
        resourceType: 'ssl',
        type: 'create',
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should update service nested route', () => {
    const serviceName = 'Test Service';
    const routeName = 'Test Route';
    const oldService: ADCSDK.Service = {
      name: serviceName,
      routes: [
        {
          name: routeName,
          uris: ['/test'],
          plugins: {
            test: {
              testKey: 'oldValue',
            },
          },
        },
      ],
    };

    const newService = structuredClone(oldService);
    newService.routes[0].plugins.test.testKey = 'newValue';

    expect(
      DifferV3.diff(
        {
          services: [structuredClone(newService)],
        },
        {
          services: [structuredClone(oldService)],
        },
      ),
    ).toEqual([
      {
        diff: [
          {
            kind: 'E',
            lhs: 'oldValue',
            path: ['plugins', 'test', 'testKey'],
            rhs: 'newValue',
          },
        ],
        newValue: newService.routes[0],
        oldValue: oldService.routes[0],
        parentId: ADCSDK.utils.generateId(serviceName),
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${routeName}`),
        resourceName: routeName,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });

  it('should update service and its nested route', () => {
    const serviceName = 'Test Service';
    const routeName = 'Test Route';
    const oldService: ADCSDK.Service = {
      name: serviceName,
      plugins: {
        test: {
          testKey: 'serviceOldValue',
        },
      },
      routes: [
        {
          name: routeName,
          uris: ['/test'],
          plugins: {
            test: {
              testKey: 'oldValue',
            },
          },
        },
      ],
    };

    const newService = structuredClone(oldService);
    newService.path_prefix = '/test';
    newService.plugins.test.testKey = 'serviceNewValue';
    newService.routes[0].plugins.test.testKey = 'newValue';

    expect(
      DifferV3.diff(
        {
          services: [structuredClone(newService)],
        },
        {
          services: [structuredClone(oldService)],
        },
      ),
    ).toEqual([
      {
        diff: [
          {
            kind: 'E',
            lhs: 'oldValue',
            path: ['plugins', 'test', 'testKey'],
            rhs: 'newValue',
          },
        ],
        newValue: newService.routes[0],
        oldValue: oldService.routes[0],
        parentId: ADCSDK.utils.generateId(serviceName),
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${routeName}`),
        resourceName: routeName,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.UPDATE,
      },
      {
        diff: [
          {
            kind: 'E',
            lhs: 'serviceOldValue',
            path: ['plugins', 'test', 'testKey'],
            rhs: 'serviceNewValue',
          },
          {
            kind: 'N',
            path: ['path_prefix'],
            rhs: '/test',
          },
        ],
        newValue: {
          name: serviceName,
          path_prefix: '/test',
          plugins: { test: { testKey: 'serviceNewValue' } },
        },
        oldValue: {
          name: serviceName,
          plugins: { test: { testKey: 'serviceOldValue' } },
        },
        resourceId: ADCSDK.utils.generateId(serviceName),
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });

  it('should keep plugins when plugins not be changed', () => {
    const serviceName = 'Test Service';
    const oldService: ADCSDK.Service = {
      name: serviceName,
      plugins: {
        test: {
          testKey: 'testValue',
          added: 'added',
        },
      },
    };

    const newService = structuredClone(oldService);
    newService.path_prefix = '/test';
    const origNewService = structuredClone(newService);

    // ensure local resource does not include default added field
    unset(newService, 'plugins.test.added');

    expect(
      DifferV3.diff(
        {
          services: [structuredClone(newService)],
        },
        {
          services: [structuredClone(oldService)],
        },
        {
          plugins: {
            test: {
              added: 'added',
            },
          },
        },
      ),
    ).toEqual([
      {
        diff: [
          {
            kind: 'N',
            path: ['path_prefix'],
            rhs: '/test',
          },
        ],
        newValue: origNewService,
        oldValue: oldService,
        resourceId: ADCSDK.utils.generateId(serviceName),
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });

  it('should selectively merge the objects in default values, for principle', () => {
    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: 'Test Service',
              //@ts-expect-error just for test
              test1: {},
            },
          ],
        },
        {
          services: [
            {
              name: 'Test Service',
              test: 'test',
              test1: {
                test2: 'test2',
              },
            },
          ],
        },
        {
          core: {
            service: {
              test: 'test',
              test1: {
                test2: 'test2',
                test3: {
                  test4: 'test4',
                },
              },
            },
          },
        },
      ),
    ).toEqual([]);
  });

  it('ensure remote metadata does not affect the differ, delete', () => {
    const serviceName = 'Test Service';
    const routeName = 'Test Route';
    const route1Name = `${routeName} 1`;
    const route2Name = `${routeName} 2`;
    const remoteServiceId = 'not_hashed_service_name';
    const remoteRouteId = 'not_hashed_route_name';
    expect(
      DifferV3.diff(
        {},
        {
          services: [
            {
              name: serviceName,
              routes: [
                {
                  name: route1Name,
                  uris: ['/test1'],
                },
                {
                  name: route2Name,
                  uris: ['/test2'],
                  metadata: { id: remoteRouteId },
                },
              ],
              metadata: {
                id: remoteServiceId,
              },
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: { name: route1Name, uris: ['/test1'] },
        parentId: remoteServiceId,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${route1Name}`),
        resourceName: route1Name,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: { name: route2Name, uris: ['/test2'] },
        parentId: remoteServiceId,
        resourceId: remoteRouteId,
        resourceName: route2Name,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: {
          name: serviceName,
          routes: [
            { name: route1Name, uris: ['/test1'] },
            { name: route2Name, uris: ['/test2'] },
          ],
        },
        resourceId: remoteServiceId,
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.DELETE,
      },
    ]);
  });

  it('ensure remote metadata does not affect the differ, update', () => {
    const serviceName = 'Test Service';
    const routeName = 'Test Route';
    const route1Name = `${routeName} 1`;
    const route2Name = `${routeName} 2`;
    const remoteServiceId = 'not_hashed_service_name';
    const remoteRouteId = 'not_hashed_route_name';
    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: serviceName,
              path_prefix: '/test',
              routes: [
                {
                  name: route1Name,
                  uris: ['/test1u'],
                },
                {
                  name: route2Name,
                  uris: ['/test2u'],
                },
              ],
            },
          ],
        },
        {
          services: [
            {
              name: serviceName,
              routes: [
                {
                  name: route1Name,
                  uris: ['/test1'],
                },
                {
                  name: route2Name,
                  uris: ['/test2'],
                  metadata: { id: remoteRouteId },
                },
              ],
              metadata: {
                id: remoteServiceId,
              },
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: { name: route1Name, uris: ['/test1'] },
        parentId: remoteServiceId,
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${route1Name}`),
        resourceName: route1Name,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: { name: route2Name, uris: ['/test2'] },
        parentId: remoteServiceId,
        resourceId: remoteRouteId,
        resourceName: route2Name,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: {
          name: serviceName,
          routes: [
            { name: route1Name, uris: ['/test1'] },
            { name: route2Name, uris: ['/test2'] },
          ],
        },
        resourceId: remoteServiceId,
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        newValue: {
          name: serviceName,
          path_prefix: '/test',
          routes: [
            { name: route1Name, uris: ['/test1u'] },
            {
              name: route2Name,
              uris: ['/test2u'],
            },
          ],
        },
        resourceId: ADCSDK.utils.generateId(serviceName),
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.CREATE,
      },
      {
        newValue: { name: route1Name, uris: ['/test1u'] },
        parentId: ADCSDK.utils.generateId(serviceName),
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${route1Name}`),
        resourceName: route1Name,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.CREATE,
      },
      {
        newValue: {
          name: route2Name,
          uris: ['/test2u'],
        },
        parentId: ADCSDK.utils.generateId(serviceName),
        resourceId: ADCSDK.utils.generateId(`${serviceName}.${route2Name}`),
        resourceName: route2Name,
        resourceType: ADCSDK.ResourceType.ROUTE,
        type: ADCSDK.EventType.CREATE,
      },
    ]);
  });

  it('ensure default values for array nested object formats are merged correctly', () => {
    const serviceName = 'Test Service';
    const newNode: ADCSDK.UpstreamNode = {
      host: '0.0.0.0',
      port: 443,
      weight: 1,
    };
    const oldNode = structuredClone(newNode);
    oldNode.priority = 0;

    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: serviceName,
              upstream: { nodes: [newNode] },
            },
          ],
        },
        {
          services: [
            {
              name: serviceName,
              upstream: { nodes: [oldNode] },
            },
          ],
        },
        {
          core: {
            service: {
              upstream: {
                nodes: [{ priority: 0 }],
              },
            },
          },
        },
      ),
    ).toEqual([]);
  });

  it('ensure route and stream route id generated currect', () => {
    const services: Array<ADCSDK.Service> = [
      {
        name: 'HTTP',
        routes: [
          {
            name: 'HTTP 1',
            uris: ['/1'],
          },
        ],
      },
      {
        name: 'Stream',
        stream_routes: [
          {
            name: 'Stream 1',
            server_port: 5432,
          },
        ],
      },
    ];

    expect(DifferV3.diff({ services }, { services }, {})).toEqual([]);
  });

  it('ensure boolean defaults are merged correctly', () => {
    const service: ADCSDK.Service = {
      name: 'HTTP',
      path_prefix: '/test',
      strip_path_prefix: false,
    };
    const oldService = structuredClone(service);
    oldService.strip_path_prefix = true;

    expect(
      DifferV3.diff(
        { services: [service] },
        { services: [oldService] },
        {
          core: {
            service: {
              strip_path_prefix: true,
            },
          },
        },
      ),
    ).toEqual([
      {
        diff: [
          { kind: 'E', lhs: true, path: ['strip_path_prefix'], rhs: false },
        ],
        newValue: service,
        oldValue: oldService,
        resourceId: ADCSDK.utils.generateId(service.name),
        resourceName: service.name,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });
});
