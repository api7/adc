import * as ADCSDK from '@api7/adc-sdk';

import { FromADC, ToADC } from '../src/transformer';
import * as typing from '../src/typing';

describe('Transformer', () => {
  describe('stream route plugins round-trip', () => {
    const plugins: ADCSDK.Plugins = {
      'ip-restriction': { blacklist: ['0.0.0.0/0'] },
    };

    it('FromADC.transformStreamRoute writes plugins', () => {
      const out = new FromADC().transformStreamRoute(
        {
          id: 'sr1',
          name: 'sr1',
          description: 'desc',
          plugins,
        } as ADCSDK.StreamRoute,
        'svc1',
      );
      expect(out.plugins).toEqual(plugins);
    });

    // Regression: ToADC.transformStreamRoute used to drop the plugins field, so
    // dumping a stream route always returned it without plugins. The differ then
    // could not detect plugin removal (local empty === remote empty), leaving
    // stale stream-route plugins on the gateway.
    it('ToADC.transformStreamRoute preserves plugins on dump', () => {
      const out = new ToADC().transformStreamRoute({
        id: 'sr1',
        name: 'sr1',
        desc: 'desc',
        service_id: 'svc1',
        stream_route_id: 'sr1',
        plugins,
        server_addr: '1.1.1.1',
        server_port: 80,
      } as typing.StreamRoute);
      expect(out.plugins).toEqual(plugins);
    });
  });

  describe('active health check req_headers round-trip', () => {
    it('ToADC.transformUpstream maps req_headers to ADC http_req_headers', () => {
      const out = new ToADC().transformUpstream({
        name: 'ups1',
        checks: {
          active: {
            type: 'http',
            req_headers: ['X-Foo: bar'],
            http_req_body: 'ping',
          },
        },
      } as typing.Upstream);
      expect(out.checks).toEqual({
        active: {
          type: 'http',
          http_req_headers: ['X-Foo: bar'],
          http_req_body: 'ping',
        },
      });
    });

    it('FromADC.transformUpstream maps ADC http_req_headers back to req_headers', () => {
      const out = new FromADC().transformUpstream({
        name: 'ups1',
        checks: {
          active: {
            type: 'http',
            http_req_headers: ['X-Foo: bar'],
            http_req_body: 'ping',
          },
        },
      } as ADCSDK.Upstream);
      expect(out.checks).toEqual({
        active: {
          type: 'http',
          req_headers: ['X-Foo: bar'],
          http_req_body: 'ping',
        },
      });
    });

    // Regression: FromADC.transformService used to cast service.upstream
    // directly to typing.Upstream instead of routing it through
    // transformUpstream, so an inline upstream's http_req_headers never got
    // mapped to API7's req_headers.
    it('FromADC.transformService maps inline upstream http_req_headers to req_headers', () => {
      const out = new FromADC().transformService({
        id: 'svc1',
        name: 'svc1',
        upstream: {
          name: 'ups1',
          checks: {
            active: {
              type: 'http',
              http_req_headers: ['X-Foo: bar'],
              http_req_body: 'ping',
            },
          },
        },
      } as ADCSDK.Service);
      expect(out.upstream?.checks).toEqual({
        active: {
          type: 'http',
          req_headers: ['X-Foo: bar'],
          http_req_body: 'ping',
        },
      });
    });
  });
});
