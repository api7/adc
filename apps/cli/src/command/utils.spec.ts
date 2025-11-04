import * as ADCSDK from '@api7/adc-sdk';

import {
  fillLabels,
  recursiveRemoveMetadataField,
  recursiveReplaceEnvVars,
} from './utils';

describe('CLI utils', () => {
  it('should fill label selector for local resources', () => {
    expect(
      fillLabels(
        {
          services: [
            {
              name: 'Test Service',
              routes: [
                {
                  name: 'Test Nested Route',
                  uris: ['/test-nested'],
                },
              ],
              stream_routes: [
                {
                  name: 'Test Nested Stream Route',
                },
              ],
            },
          ],
          ssls: [
            {
              snis: ['test.com'],
              certificates: [],
            },
          ],
          consumers: [
            {
              username: 'alice',
            },
          ],
          routes: [
            {
              name: 'Test Route',
              uris: ['/test'],
            },
          ],
          stream_routes: [
            {
              name: 'Test Stream Route',
            },
          ],
        },
        { test1: 'test1', test2: 'test2' },
      ),
    ).toEqual({
      consumers: [
        { labels: { test1: 'test1', test2: 'test2' }, username: 'alice' },
      ],
      routes: [
        {
          labels: { test1: 'test1', test2: 'test2' },
          name: 'Test Route',
          uris: ['/test'],
        },
      ],
      services: [
        {
          labels: { test1: 'test1', test2: 'test2' },
          name: 'Test Service',
          routes: [
            {
              labels: { test1: 'test1', test2: 'test2' },
              name: 'Test Nested Route',
              uris: ['/test-nested'],
            },
          ],
          stream_routes: [
            {
              labels: { test1: 'test1', test2: 'test2' },
              name: 'Test Nested Stream Route',
            },
          ],
        },
      ],
      ssls: [
        {
          certificates: [],
          labels: { test1: 'test1', test2: 'test2' },
          snis: ['test.com'],
        },
      ],
      stream_routes: [
        {
          labels: { test1: 'test1', test2: 'test2' },
          name: 'Test Stream Route',
        },
      ],
    });
  });

  it('should fill label selector for local resources, append', () => {
    expect(
      fillLabels(
        {
          services: [
            {
              name: 'Test Service',
              labels: { test: 'test' },
              routes: [
                {
                  name: 'Test Nested Route',
                  labels: { test: 'test' },
                  uris: ['/test-nested'],
                },
              ],
              stream_routes: [
                {
                  name: 'Test Nested Stream Route',
                  labels: { test: 'test' },
                },
              ],
            },
          ],
          ssls: [
            {
              snis: ['test.com'],
              labels: { test: 'test' },
              certificates: [],
            },
          ],
          consumers: [
            {
              username: 'alice',
              labels: { test: 'test' },
            },
          ],
          routes: [
            {
              name: 'Test Route',
              labels: { test: 'test' },
              uris: ['/test'],
            },
          ],
          stream_routes: [
            {
              name: 'Test Stream Route',
              labels: { test: 'test' },
            },
          ],
        },
        { test1: 'test1', test2: 'test2' },
      ),
    ).toEqual({
      consumers: [
        {
          labels: { test: 'test', test1: 'test1', test2: 'test2' },
          username: 'alice',
        },
      ],

      routes: [
        {
          labels: { test: 'test', test1: 'test1', test2: 'test2' },
          name: 'Test Route',
          uris: ['/test'],
        },
      ],
      services: [
        {
          labels: { test: 'test', test1: 'test1', test2: 'test2' },
          name: 'Test Service',
          routes: [
            {
              labels: { test: 'test', test1: 'test1', test2: 'test2' },
              name: 'Test Nested Route',
              uris: ['/test-nested'],
            },
          ],
          stream_routes: [
            {
              labels: { test: 'test', test1: 'test1', test2: 'test2' },
              name: 'Test Nested Stream Route',
            },
          ],
        },
      ],
      ssls: [
        {
          certificates: [],
          labels: { test: 'test', test1: 'test1', test2: 'test2' },
          snis: ['test.com'],
        },
      ],
      stream_routes: [
        {
          labels: { test: 'test', test1: 'test1', test2: 'test2' },
          name: 'Test Stream Route',
        },
      ],
    });
  });

  it('should remove metadata from dump result', () => {
    const config = {
      services: [
        {
          name: 'TestService1',
          metadata: { id: 'test_service1' },
          routes: [
            {
              name: 'TestRoute',
              uris: ['/test'],
              metadata: { id: 'test_route' },
            },
          ],
        },
        {
          name: 'TestService2',
          metadata: { id: 'test_service2' },
          stream_routes: [
            {
              name: 'TestStreamRoute',
              metadata: { id: 'test_stream_route' },
            },
          ],
        },
      ],
      ssls: [
        {
          snis: ['test'],
          certificates: [],
          metadata: { id: 'test_ssl' },
        },
      ],
    } as unknown as ADCSDK.Configuration;
    recursiveRemoveMetadataField(config);
    expect(config).toEqual({
      services: [
        {
          name: 'TestService1',
          routes: [
            {
              name: 'TestRoute',
              uris: ['/test'],
            },
          ],
        },
        {
          name: 'TestService2',
          stream_routes: [{ name: 'TestStreamRoute' }],
        },
      ],
      ssls: [{ certificates: [], snis: ['test'] }],
    });
  });

  describe('Environment Variables', () => {
    it('mock config', () => {
      const config: ADCSDK.Configuration = {
        services: [
          {
            name: 'Test ${NAME}',
            routes: [
              {
                name: 'Test ${NAME}',
                uris: ['/test/${NAME}'],
              },
            ],
          },
          {
            name: 'Test escape \\${NAME}',
          },
        ],
        consumers: [
          {
            username: 'TEST_${NAME}',
            plugins: {
              'key-auth': {
                key: '${SECRET}',
              },
            },
          },
        ],
        ssls: [
          {
            snis: ['test.com'],
            certificates: [
              {
                certificate: '${CERT}',
                key: '${KEY}',
              },
            ],
          },
        ],
        global_rules: {
          // object key contains variables will not be parsed
          '${GLOBAL_PLUGIN}': {
            key: '${SECRET}',
          },
        },
        plugin_metadata: {
          'file-logger': {
            log_format: {
              note: '${NOTE}',
            },
          },
        },
      };
      expect(
        recursiveReplaceEnvVars(config, {
          NAME: 'name',
          SECRET: 'secret',
          CERT: '-----',
          KEY: '-----',
          NOTE: 'note',
        }),
      ).toEqual({
        consumers: [
          { plugins: { 'key-auth': { key: 'secret' } }, username: 'TEST_name' },
        ],
        global_rules: { '${GLOBAL_PLUGIN}': { key: 'secret' } },
        plugin_metadata: { 'file-logger': { log_format: { note: 'note' } } },
        services: [
          {
            name: 'Test name',
            routes: [{ name: 'Test name', uris: ['/test/name'] }],
          },
          {
            name: 'Test escape ${NAME}',
          },
        ],
        ssls: [
          {
            certificates: [{ certificate: '-----', key: '-----' }],
            snis: ['test.com'],
          },
        ],
      });
    });
  });
});
