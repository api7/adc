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
          code: 'invalid_union',
          errors: [
            [
              {
                code: 'too_small',
                inclusive: true,
                message: 'Too small: expected string to have >=128 characters',
                minimum: 128,
                origin: 'string',
                path: [],
              },
            ],
            [
              {
                code: 'invalid_format',
                format: 'regex',
                message:
                  'Invalid string: must match pattern /^\\$(secret|env):\\/\\//',
                origin: 'string',
                path: [],
                pattern: '/^\\$(secret|env):\\/\\//',
              },
            ],
          ],
          message: 'Must be a certificate string or a secret reference',
          path: ['ssls', 0, 'certificates', 0, 'certificate'],
        },
        {
          code: 'invalid_union',
          errors: [
            [
              {
                code: 'too_small',
                inclusive: true,
                message: 'Too small: expected string to have >=32 characters',
                minimum: 32,
                origin: 'string',
                path: [],
              },
            ],
            [
              {
                code: 'invalid_format',
                format: 'regex',
                message:
                  'Invalid string: must match pattern /^\\$(secret|env):\\/\\//',
                origin: 'string',
                path: [],
                pattern: '/^\\$(secret|env):\\/\\//',
              },
            ],
          ],
          message:
            'Must be a certificate private key string or a secret reference',
          path: ['ssls', 0, 'certificates', 0, 'key'],
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
        expect(result.error.issues).toEqual(item.errors);
      }
    });
  });
});
