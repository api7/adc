import * as ADCSDK from '@api7/adc-sdk';
import { InvalidArgumentError } from 'commander';

import { parseLabelSelector } from './helper';
import {
  fillLabels,
  filterConfiguration,
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
        } as ADCSDK.Configuration,
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
        } as ADCSDK.Configuration,
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

  it('should filter configuration by exact label key match', () => {
    const [filtered, removed] = filterConfiguration(
      {
        services: [
          {
            name: 'match-by-name',
            labels: { name: 'yanglao_wx_pgm' },
          },
          {
            name: 'should-not-match-appname',
            labels: { appname: 'yanglao_wx_pgm' },
          },
          {
            name: 'should-not-match-different-value',
            labels: { name: 'other' },
          },
        ],
      } as ADCSDK.Configuration,
      { name: 'yanglao_wx_pgm' },
    );

    expect(filtered.services).toEqual([
      {
        name: 'match-by-name',
        labels: { name: 'yanglao_wx_pgm' },
      },
    ]);
    expect(removed.services).toEqual([
      {
        name: 'should-not-match-appname',
        labels: { appname: 'yanglao_wx_pgm' },
      },
      {
        name: 'should-not-match-different-value',
        labels: { name: 'other' },
      },
    ]);
  });

  it('should parse label selectors in key=value format only', () => {
    expect(parseLabelSelector('name=yanglao_wx_pgm')).toEqual({
      name: 'yanglao_wx_pgm',
    });
    expect(parseLabelSelector('release=2026-03-13T12:00:00Z')).toEqual({
      release: '2026-03-13T12:00:00Z',
    });
    expect(parseLabelSelector('url=http://a')).toEqual({
      url: 'http://a',
    });
    expect(
      parseLabelSelector('name=yanglao_wx_pgm,team=adc', { env: 'prod' }),
    ).toEqual({
      env: 'prod',
      name: 'yanglao_wx_pgm',
      team: 'adc',
    });
  });

  it('should reject invalid label selector formats', () => {
    expect(() => parseLabelSelector('name:yanglao_wx_pgm')).toThrow(
      InvalidArgumentError,
    );
    expect(() => parseLabelSelector('name=')).toThrow(InvalidArgumentError);
    expect(() => parseLabelSelector('=yanglao_wx_pgm')).toThrow(
      InvalidArgumentError,
    );
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
