import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3.js';

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

    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: serviceName,
              routes: [
                {
                  name: routeName,
                  uris: ['/test'],
                  plugins: {
                    test: {
                      testKey: 'newValue',
                    },
                  },
                },
              ],
            },
          ],
        },
        {
          services: [
            {
              id: ADCSDK.utils.generateId(serviceName),
              name: serviceName,
              routes: [
                {
                  id: ADCSDK.utils.generateId(`${serviceName}.${routeName}`),
                  name: routeName,
                  uris: ['/test'],
                  plugins: {
                    test: {
                      testKey: 'oldValue',
                    },
                  },
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
            lhs: 'oldValue',
            path: ['plugins', 'test', 'testKey'],
            rhs: 'newValue',
          },
        ],
        newValue: {
          name: routeName,
          uris: ['/test'],
          plugins: {
            test: {
              testKey: 'newValue',
            },
          },
        },
        oldValue: {
          name: routeName,
          uris: ['/test'],
          plugins: {
            test: {
              testKey: 'oldValue',
            },
          },
        },
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
    const serviceId = ADCSDK.utils.generateId(serviceName);
    const routeName = 'Test Route';
    const routeId = ADCSDK.utils.generateId(`${serviceName}.${routeName}`);

    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: serviceName,
              path_prefix: '/test',
              plugins: {
                test: {
                  testKey: 'serviceNewValue',
                },
              },
              routes: [
                {
                  name: routeName,
                  uris: ['/test'],
                  plugins: {
                    test: {
                      testKey: 'newValue',
                    },
                  },
                },
              ],
            },
          ],
        },
        {
          services: [
            {
              id: serviceId,
              name: serviceName,
              plugins: {
                test: {
                  testKey: 'serviceOldValue',
                },
              },
              routes: [
                {
                  id: routeId,
                  name: routeName,
                  uris: ['/test'],
                  plugins: {
                    test: {
                      testKey: 'oldValue',
                    },
                  },
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
            lhs: 'oldValue',
            path: ['plugins', 'test', 'testKey'],
            rhs: 'newValue',
          },
        ],
        newValue: {
          name: routeName,
          uris: ['/test'],
          plugins: {
            test: {
              testKey: 'newValue',
            },
          },
        },
        oldValue: {
          name: routeName,
          uris: ['/test'],
          plugins: {
            test: {
              testKey: 'oldValue',
            },
          },
        },
        parentId: serviceId,
        resourceId: routeId,
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
        resourceId: serviceId,
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });

  it('should keep plugins when plugins not be changed', () => {
    const serviceName = 'Test Service';
    const serviceId = ADCSDK.utils.generateId(serviceName);

    expect(
      DifferV3.diff(
        {
          services: [
            {
              name: serviceName,
              path_prefix: '/test',
              plugins: {
                test: {
                  testKey: 'testValue',
                },
              },
            },
          ],
        },
        {
          services: [
            {
              id: serviceId,
              name: serviceName,
              plugins: {
                test: {
                  testKey: 'testValue',
                  added: 'added',
                },
              },
            },
          ],
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
        newValue: {
          name: serviceName,
          path_prefix: '/test',
          plugins: {
            test: {
              testKey: 'testValue',
              added: 'added',
            },
          },
        },
        oldValue: {
          name: serviceName,
          plugins: {
            test: {
              testKey: 'testValue',
              added: 'added',
            },
          },
        },
        resourceId: serviceId,
        resourceName: serviceName,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });

  it('should selectively merge the objects in default values, for principle', () => {
    const serviceName = 'Test Service';
    const serviceId = ADCSDK.utils.generateId(serviceName);
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
              id: serviceId,
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
              id: ADCSDK.utils.generateId(serviceName),
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
    const newServices: Array<ADCSDK.Service> = [
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

    const oldServices = structuredClone(newServices);
    oldServices[0].id = ADCSDK.utils.generateId('HTTP');
    oldServices[0].routes![0].id = ADCSDK.utils.generateId('HTTP.HTTP 1');
    oldServices[1].id = ADCSDK.utils.generateId('Stream');
    oldServices[1].stream_routes![0].id =
      ADCSDK.utils.generateId('Stream.Stream 1');

    expect(
      DifferV3.diff({ services: newServices }, { services: oldServices }, {}),
    ).toEqual([]);
  });

  it('ensure boolean defaults are merged correctly', () => {
    const service: ADCSDK.Service = {
      name: 'HTTP',
      path_prefix: '/test',
      strip_path_prefix: false,
    };
    const oldService = structuredClone(service);
    oldService.id = ADCSDK.utils.generateId('HTTP');
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
        oldValue: { ...service, strip_path_prefix: true },
        resourceId: ADCSDK.utils.generateId(service.name),
        resourceName: service.name,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });
});
