import { toADC } from '../src/transformer';
import type * as typing from '../src/typing';

describe('Transformer', () => {
  it('should map active health check req_headers to ADC http_req_headers', () => {
    const input: typing.APISIXStandalone = {
      services: [
        {
          modifiedIndex: 1,
          id: 'svc-1',
          name: 'svc-1',
          upstream_id: 'ups-1',
        },
      ],
      upstreams: [
        {
          modifiedIndex: 1,
          id: 'ups-1',
          name: 'ups-1',
          checks: {
            active: {
              type: 'http',
              req_headers: ['X-Foo: bar'],
              http_req_body: 'ping',
            },
          },
        },
      ],
    };

    const result = toADC(input);

    expect(result.services?.[0].upstream?.checks).toEqual({
      active: {
        type: 'http',
        http_req_headers: ['X-Foo: bar'],
        http_req_body: 'ping',
      },
    });
  });
});
