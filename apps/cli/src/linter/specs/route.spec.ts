import * as ADCSDK from '@api7/adc-sdk';

import { check } from '../';

describe('Route Linter', () => {
  const cases = [
    {
      name: 'should check route vars',
      input: {
        services: [
          {
            name: 'test',
            routes: [
              {
                name: 'test',
                uris: ['/test'],
                vars: [
                  'AND',
                  ['arg_version', '==', 'v2'],
                  [
                    'OR',
                    ['arg_action', '==', 'signup'],
                    ['arg_action', '==', 'subscribe'],
                  ],
                ],
              },
            ],
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
      errors: [],
    },
    ...[
      {
        name: 'should accept a stream route with only sni',
        route: { name: 'test', sni: 'a.example.com' },
        expect: true,
      },
      {
        name: 'should accept a stream route with only snis',
        route: { name: 'test', snis: ['a.example.com'] },
        expect: true,
      },
      {
        name: 'should reject a stream route with both sni and snis',
        route: {
          name: 'test',
          sni: 'a.example.com',
          snis: ['b.example.com'],
        },
        expect: false,
      },
    ].map((c) => ({
      name: c.name,
      input: {
        services: [{ name: 'test', stream_routes: [c.route] }],
      } as ADCSDK.Configuration,
      expect: c.expect,
      errors: c.expect
        ? []
        : [
            {
              code: 'custom',
              message: 'Stream route must not specify both sni and snis',
              path: ['services', 0, 'stream_routes', 0],
            },
          ],
    })),
  ];

  // test cases runner
  cases.forEach((item) => {
    it(item.name, () => {
      const result = check(item.input);
      expect(result.success).toEqual(item.expect);
      if (!item.expect && !result.success) {
        expect(result.error.issues).toEqual(item.errors);
      }
    });
  });
});
