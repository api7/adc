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
