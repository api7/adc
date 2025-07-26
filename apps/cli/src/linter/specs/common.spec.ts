import * as ADCSDK from '@api7/adc-sdk';

import { check } from '..';

describe('Common Linter', () => {
  const cases = [
    {
      name: 'should check name/description length (length <= 64 * 1024)',
      input: {
        services: [
          {
            name: ''.padEnd(64 * 1024, '0'),
            description: ''.padEnd(64 * 1024, '0'),
            routes: [],
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
      errors: [],
    },
    {
      name: 'should check name/description length (length > 64 * 1024)',
      input: {
        services: [
          {
            name: ''.padEnd(64 * 1024 + 1, '0'),
            description: ''.padEnd(64 * 1024 + 1, '0'),
            routes: [],
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'too_big',
          inclusive: true,
          maximum: 65536,
          message: 'Too big: expected string to have <=65536 characters',
          origin: 'string',
          path: ['services', 0, 'name'],
        },
        {
          code: 'too_big',
          inclusive: true,
          maximum: 65536,
          message: 'Too big: expected string to have <=65536 characters',
          origin: 'string',
          path: ['services', 0, 'description'],
        },
      ],
    },
    {
      name: 'should check custom resource id',
      input: {
        services: [
          {
            id: 'custom-service',
            name: 'test',
            routes: [
              {
                id: 'custom-route',
                name: 'test',
                uris: ['/test'],
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should check id length (length <= 256)',
      input: {
        services: [
          {
            id: ''.padEnd(256, '0'),
            name: 'name',
            routes: [],
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
      errors: [],
    },
    {
      name: 'should check id length (length > 256)',
      input: {
        services: [
          {
            id: ''.padEnd(257, '0'),
            name: 'name',
            routes: [],
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'too_big',
          inclusive: true,
          maximum: 256,
          message: 'Too big: expected string to have <=256 characters',
          origin: 'string',
          path: ['services', 0, 'id'],
        },
      ],
    },
    {
      name: 'should automatically handle numeric field parsing',
      //@ts-expect-error for test
      input: {
        services: [
          {
            name: 'name',
            upstream: {
              type: 'roundrobin',
              nodes: [
                {
                  host: 'httpbin.org',
                  port: '443', // string is automatically parsed as an integer
                  weight: '100',
                },
              ],
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should automatically handle numeric field parsing (fail)',
      //@ts-expect-error for test
      input: {
        services: [
          {
            name: 'name',
            upstream: {
              type: 'roundrobin',
              nodes: [
                {
                  host: 'httpbin.org',
                  port: '443.1', // require an integer but enter a float
                  weight: 100,
                },
              ],
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'invalid_type',
          expected: 'int',
          format: 'safeint',
          message: 'Invalid input: expected int, received number',
          path: ['services', 0, 'upstream', 'nodes', 0, 'port'],
        },
      ],
    },
  ];

  // test cases runner
  cases.forEach((item) => {
    it(item.name, () => {
      const result = check(item.input);
      expect(result.success).toEqual(item.expect);
      if (!item.expect) {
        expect(result.error.issues).toEqual(item.errors);
      }
    });
  });
});
