import * as ADCSDK from '@api7/adc-sdk';

import { check } from '../';

describe('Upstream Linter', () => {
  const cases = [
    {
      name: 'should check either nodes or discovery (neither)',
      input: {
        services: [
          {
            name: 'No_Node_And_Discovery',
            upstream: {},
          },
        ],
      } as ADCSDK.Configuration,
      expect: false,
      errors: [
        {
          code: 'custom',
          message:
            'Upstream must either explicitly specify nodes or use service discovery and not both',
          path: ['services', 0, 'upstream'],
        },
      ],
    },
    {
      name: 'should check either nodes or discovery (with nodes)',
      input: {
        services: [
          {
            name: 'No_Node_And_Discovery',
            upstream: {
              nodes: [
                {
                  host: '1.1.1.1',
                  port: 443,
                  weight: 100,
                },
              ],
            },
          },
        ],
      } as ADCSDK.Configuration,
      expect: true,
    },
    {
      name: 'should check either nodes or discovery (with discovery and service name)',
      input: {
        services: [
          {
            name: 'No_Node_And_Discovery',
            upstream: {
              discovery_type: 'mock',
              service_name: 'service_mock',
            },
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
