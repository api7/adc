import * as ADCSDK from '@api7/adc-sdk';

import { check } from '..';

describe('Consumer Linter', () => {
  const cases = [
    {
      name: 'should check consumer credentials (success)',
      input: {
        consumers: [
          {
            username: 'jack',
            credentials: [
              {
                name: 'jack-1',
                type: 'key-auth',
                config: {
                  key: 'jack',
                },
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
      errors: [],
    },
    {
      name: 'should check consumer credentials (miss config)',
      input: {
        consumers: [
          {
            username: 'jack',
            credentials: [
              {
                name: 'jack-1',
                type: 'key-auth',
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'invalid_type',
          expected: 'object',
          message: 'Invalid input: expected object, received undefined',
          path: ['consumers', 0, 'credentials', 0, 'config'],
        },
      ],
    },
    {
      name: 'should check consumer credentials (unsupported type)',
      //@ts-expect-error for test
      input: {
        consumers: [
          {
            username: 'jack',
            credentials: [
              {
                name: 'jack-1',
                type: 'none-auth',
                config: {},
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'custom',
          message:
            'Consumer credential only supports "key-auth", "basic-auth", "jwt-auth" and "hmac-auth" types',
          path: ['consumers', 0, 'credentials', 0, 'type'],
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
