import * as ADCSDK from '@api7/adc-sdk';
import { globalAgent as httpAgent } from 'node:http';

import { BackendAPI7 } from '../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  generateHTTPSAgent,
  syncEvents,
  updateEvent,
} from './support/utils';

describe('Sync and Dump - stream route plugins', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER!,
      token: process.env.TOKEN!,
      tlsSkipVerify: true,
      gatewayGroup: process.env.GATEWAY_GROUP,
      cacheKey: 'default',
      httpAgent,
      httpsAgent: generateHTTPSAgent(),
    });
  });

  // Regression: ToADC.transformStreamRoute used to drop the plugins field, so a
  // dumped stream route always came back without plugins. The "Dump preserves the
  // stream route plugins" assertion below fails on the buggy code and passes once
  // plugins are mapped on the dump path.
  describe('Stream route plugin round-trip and removal', () => {
    const serviceName = 'stream-service';
    const service = {
      name: serviceName,
      upstream: {
        scheme: 'tcp',
        nodes: [{ host: 'httpbin.org', port: 80, weight: 100 }],
      },
    } as ADCSDK.Service;

    const streamRouteName = 'stream-route';
    const plugins: ADCSDK.Plugins = {
      'ip-restriction': { whitelist: ['127.0.0.0/24'] },
    };
    const streamRoute = {
      name: streamRouteName,
      plugins,
    } as ADCSDK.StreamRoute;

    it('Create stream service and stream route with plugins', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.SERVICE, serviceName, service),
        createEvent(
          ADCSDK.ResourceType.STREAM_ROUTE,
          streamRouteName,
          streamRoute,
          serviceName,
        ),
      ]));

    it('Dump preserves the stream route plugins', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      const svc = result.services?.find(
        (item: ADCSDK.Service) => item.name === serviceName,
      );
      expect(svc?.stream_routes).toHaveLength(1);
      expect(svc?.stream_routes?.[0].plugins).toMatchObject(plugins);
    });

    it('Remove all plugins from the stream route', async () =>
      syncEvents(backend, [
        updateEvent(
          ADCSDK.ResourceType.STREAM_ROUTE,
          streamRouteName,
          { ...streamRoute, plugins: {} },
          serviceName,
        ),
      ]));

    it('Dump reflects the plugin removal', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      const svc = result.services?.find(
        (item: ADCSDK.Service) => item.name === serviceName,
      );
      expect(svc?.stream_routes).toHaveLength(1);
      expect(svc?.stream_routes?.[0].plugins ?? {}).toEqual({});
    });

    it('Delete', async () =>
      syncEvents(backend, [
        deleteEvent(
          ADCSDK.ResourceType.STREAM_ROUTE,
          streamRouteName,
          serviceName,
        ),
        deleteEvent(ADCSDK.ResourceType.SERVICE, serviceName),
      ]));
  });
});
