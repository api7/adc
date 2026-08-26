import { FromADC, ToADC } from '../src/transformer';

describe('Transformer', () => {
  it('should transform upstream nodes to array', () => {
    const toADC = new ToADC();
    expect(
      toADC.transformUpstream({
        nodes: {
          '127.0.0.1:5432': 100,
        },
      }),
    ).toEqual({ nodes: [{ host: '127.0.0.1', port: 5432, weight: 100 }] });
  });

  it('should map active health check req_headers to ADC http_req_headers', () => {
    const toADC = new ToADC();
    expect(
      toADC.transformUpstream({
        checks: {
          active: {
            type: 'http',
            req_headers: ['X-Foo: bar'],
            http_req_body: 'ping',
          },
        },
      }),
    ).toEqual({
      checks: {
        active: {
          type: 'http',
          http_req_headers: ['X-Foo: bar'],
          http_req_body: 'ping',
        },
      },
    });
  });

  it('should map ADC http_req_headers back to active health check req_headers', () => {
    const fromADC = new FromADC();
    expect(
      fromADC.transformUpstream({
        checks: {
          active: {
            type: 'http',
            http_req_headers: ['X-Foo: bar'],
            http_req_body: 'ping',
          },
        },
      }),
    ).toEqual({
      checks: {
        active: {
          type: 'http',
          req_headers: ['X-Foo: bar'],
          http_req_body: 'ping',
        },
      },
    });
  });

  it('should fall back consumer credential name to id when APISIX returns none', () => {
    const toADC = new ToADC();
    expect(
      toADC.transformConsumerCredential({
        id: 'jack-key',
        plugins: { 'key-auth': { key: 'secret' } },
      }),
    ).toEqual({
      id: 'jack-key',
      name: 'jack-key',
      type: 'key-auth',
      config: { key: 'secret' },
    });
  });
});
