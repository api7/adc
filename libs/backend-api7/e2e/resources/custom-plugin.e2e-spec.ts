import * as ADCSDK from '@api7/adc-sdk';
import { globalAgent as httpAgent } from 'node:http';

import { BackendAPI7 } from '../../src';
import {
  createEvent,
  deleteEvent,
  dumpConfiguration,
  generateHTTPSAgent,
  syncEvents,
  updateEvent,
} from '../support/utils';

describe('Custom Plugin E2E', () => {
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

  describe('Sync and dump custom plugins', () => {
    const pluginName = 'e2e-custom-plugin';
    const pluginContent = [
      'local core = require("apisix.core")',
      'local schema = { type = "object", properties = {} }',
      `local _M = { version = 0.1, priority = 0, name = "${pluginName}", schema = schema }`,
      'function _M.check_schema(conf) return core.schema.check(schema, conf) end',
      'function _M.access(conf, ctx) end',
      'return _M',
    ].join('\n');
    const plugin = {
      name: pluginName,
      content: pluginContent,
      description: 'created by e2e',
    } as ADCSDK.CustomPlugin;

    it('Create custom plugin', async () =>
      syncEvents(backend, [
        createEvent(ADCSDK.ResourceType.CUSTOM_PLUGIN, pluginName, plugin),
      ]));

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.custom_plugins).toHaveLength(1);
      expect(result.custom_plugins?.[0]).toMatchObject({
        name: pluginName,
        content: pluginContent,
      });
    });

    it('Update custom plugin (description changed)', async () => {
      plugin.description = 'updated by e2e';
      await syncEvents(backend, [
        updateEvent(ADCSDK.ResourceType.CUSTOM_PLUGIN, pluginName, plugin),
      ]);
    });

    it('Dump again (description updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.custom_plugins?.[0]).toMatchObject({
        name: pluginName,
        description: 'updated by e2e',
      });
    });

    it('Delete custom plugin', async () =>
      syncEvents(backend, [
        deleteEvent(ADCSDK.ResourceType.CUSTOM_PLUGIN, pluginName),
      ]));

    it('Dump again (custom plugin should not exist)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.custom_plugins ?? []).toHaveLength(0);
    });
  });
});
