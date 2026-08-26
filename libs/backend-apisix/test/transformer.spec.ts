import * as ADCSDK from '@api7/adc-sdk';

import { FromADC, ToADC } from '../src/transformer';
import * as typing from '../src/typing';

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

  describe('stream route name persistence', () => {
    const streamRoute = { name: 'my-stream-route' } as ADCSDK.StreamRoute;

    it('should omit the name entirely in unsupported mode', () => {
      const fromADC = new FromADC();
      const wire = fromADC.transformStreamRoute(
        streamRoute,
        'svc1',
        'unsupported',
      );
      expect(wire.name).toBeUndefined();
      expect(wire.labels).toBeUndefined();
    });

    it('should inject the magic label in label mode', () => {
      const fromADC = new FromADC();
      const wire = fromADC.transformStreamRoute(streamRoute, 'svc1', 'label');
      expect(wire.name).toBeUndefined();
      expect(wire.labels).toEqual({ __ADC_NAME: 'my-stream-route' });
    });

    it('should use the native name field in native mode', () => {
      const fromADC = new FromADC();
      const wire = fromADC.transformStreamRoute(streamRoute, 'svc1', 'native');
      expect(wire.name).toEqual('my-stream-route');
      expect(wire.labels).toBeUndefined();
    });

    it('should prefer the native name over the magic label when reading', () => {
      const toADC = new ToADC();
      expect(
        toADC.transformStreamRoute({
          id: 'sr1',
          name: 'native-name',
          labels: { __ADC_NAME: 'stale-label-name' },
        } as typing.StreamRoute),
      ).toMatchObject({ name: 'native-name' });
    });

    it('should fall back to the magic label when there is no native name', () => {
      const toADC = new ToADC();
      expect(
        toADC.transformStreamRoute({
          id: 'sr1',
          labels: { __ADC_NAME: 'my-stream-route' },
        } as typing.StreamRoute),
      ).toMatchObject({ name: 'my-stream-route' });
    });

    it('should fall back to id when neither a name nor the magic label exists', () => {
      const toADC = new ToADC();
      expect(
        toADC.transformStreamRoute({ id: 'sr1' } as typing.StreamRoute),
      ).toMatchObject({ name: 'sr1' });
    });
  });
});
