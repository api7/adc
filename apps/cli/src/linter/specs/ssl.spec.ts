import * as ADCSDK from '@api7/adc-sdk';

import { check } from '../';

describe('SSL Linter', () => {
  const cases = [
    {
      name: 'should check for too short certificates and keys',
      input: {
        ssls: [
          {
            snis: ['test.com'],
            certificates: [
              {
                certificate: 'short',
                key: 'short',
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'too_small',
          exact: false,
          inclusive: true,
          message: 'String must contain at least 128 character(s)',
          minimum: 128,
          path: ['ssls', 0, 'certificates', 0, 'certificate'],
          type: 'string',
        },
        {
          code: 'too_small',
          exact: false,
          inclusive: true,
          message: 'String must contain at least 32 character(s)',
          minimum: 32,
          path: ['ssls', 0, 'certificates', 0, 'key'],
          type: 'string',
        },
      ],
    },
    {
      name: 'should check for dataplane env ref certificates and keys',
      input: {
        ssls: [
          {
            snis: ['test.com'],
            certificates: [
              {
                certificate: '$env://CERT',
                key: '$env://CERT_KEY',
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should check for dataplane secret ref certificates and keys',
      input: {
        ssls: [
          {
            snis: ['test.com'],
            certificates: [
              {
                certificate: '$secret://vault/test.com/cert',
                key: '$secret://vault/test.com/key',
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
