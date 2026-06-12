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
});
