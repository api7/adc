import type * as ADCSDK from '@api7/adc-sdk';
import type { AxiosInstance } from 'axios';
import { Subject } from 'rxjs';
import { SemVer } from 'semver';

import { Operator } from '../src/operator';
import type * as typing from '../src/typing';

describe('Operator', () => {
  const newOperator = () =>
    new Operator({
      cacheKey: 'test',
      client: {} as AxiosInstance,
      serverTokenMap: new Map(),
      version: new SemVer('3.9.0'),
      eventSubject: new Subject(),
      oldRawConfiguration: {},
    });

  it('should map ADC http_req_headers back to active health check req_headers', () => {
    const operator = newOperator() as unknown as {
      fromADCUpstream: (
        res: ADCSDK.Upstream,
        parentId?: string,
      ) => typing.Upstream;
    };

    const result = operator.fromADCUpstream({
      checks: {
        active: {
          type: 'http',
          http_req_headers: ['X-Foo: bar'],
          http_req_body: 'ping',
        },
      },
    } as ADCSDK.Upstream);

    expect(result.checks).toEqual({
      active: {
        type: 'http',
        req_headers: ['X-Foo: bar'],
        http_req_body: 'ping',
      },
    });
  });
});
