import * as ADCSDK from '@api7/adc-sdk';

import { check } from '../';

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
          exact: false,
          inclusive: true,
          maximum: 65536,
          message: 'String must contain at most 65536 character(s)',
          path: ['services', 0, 'name'],
          type: 'string',
        },
        {
          code: 'too_big',
          exact: false,
          inclusive: true,
          maximum: 65536,
          message: 'String must contain at most 65536 character(s)',
          path: ['services', 0, 'description'],
          type: 'string',
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
  ];

  // test cases runner
  cases.forEach((item) => {
    it(item.name, () => {
      const result = check(item.input);
      expect(result.success).toEqual(item.expect);
      if (!item.expect) {
        expect(result.error.errors).toEqual(item.errors);
      }
    });
  });
});
