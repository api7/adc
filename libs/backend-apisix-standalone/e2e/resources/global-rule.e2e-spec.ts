import { Differ } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';

import { BackendAPISIXStandalone } from '../../src';
import { defaultBackendOptions, server1, token1 } from '../support/constants';
import { dumpConfiguration, restartAPISIX, syncEvents } from '../support/utils';

describe('Global Rule E2E', () => {
  let backend: BackendAPISIXStandalone;

  beforeAll(async () => {
    await restartAPISIX();
    backend = new BackendAPISIXStandalone({
      server: server1,
      token: token1,
      cacheKey: 'default',
      ...defaultBackendOptions,
    });
  });

  describe('Sync and dump global rules', () => {
    const plugin1Name = 'request-id';
    const plugin1 = {} as ADCSDK.GlobalRule;
    const plugin2Name = 'prometheus';
    const plugin2 = { prefer_name: true } as unknown as ADCSDK.GlobalRule;

    it('Initialize cache', () =>
      expect(dumpConfiguration(backend)).resolves.not.toThrow());

    it('Create global rules', async () => {
      const events = Differ.diff(
        { global_rules: { [plugin1Name]: plugin1, [plugin2Name]: plugin2 } },
        {},
      );
      expect(events).toHaveLength(2);
      expect(events.every((e) => e.type === ADCSDK.EventType.CREATE)).toBe(true);
      await syncEvents(backend, events);
    });

    it('Dump', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.global_rules!)).toHaveLength(2);
      expect(result.global_rules![plugin1Name]).toMatchObject(plugin1);
      expect(result.global_rules![plugin2Name]).toMatchObject(plugin2);
    });

    it('Second sync is a no-op (idempotency regression test for #489)', async () => {
      const { global_rules_conf_version: versionBefore } = (
        await axios.get(`${server1}/apisix/admin/configs`, {
          headers: { 'X-API-KEY': token1 },
        })
      ).data;

      const events = Differ.diff(
        { global_rules: { [plugin1Name]: plugin1, [plugin2Name]: plugin2 } },
        await dumpConfiguration(backend),
      );
      expect(events).toHaveLength(0);
      await syncEvents(backend, events);

      const { global_rules_conf_version: versionAfter } = (
        await axios.get(`${server1}/apisix/admin/configs`, {
          headers: { 'X-API-KEY': token1 },
        })
      ).data;
      expect(versionAfter).toEqual(versionBefore);
    });

    it('Update plugin1', async () => {
      const updatedPlugin1 = { enable: false } as unknown as ADCSDK.GlobalRule;
      const events = Differ.diff(
        {
          global_rules: {
            [plugin1Name]: updatedPlugin1,
            [plugin2Name]: plugin2,
          },
        },
        await dumpConfiguration(backend),
      );
      expect(events).toHaveLength(1);
      expect(events[0].type).toBe(ADCSDK.EventType.UPDATE);
      expect(events[0].resourceId).toBe(plugin1Name);
      await syncEvents(backend, events);
    });

    it('Dump again (plugin1 updated)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(result.global_rules![plugin1Name]).toMatchObject({ enable: false });
      expect(result.global_rules![plugin2Name]).toMatchObject(plugin2);
    });

    it('Delete all global rules', async () => {
      const events = Differ.diff({}, await dumpConfiguration(backend));
      expect(events).toHaveLength(2);
      expect(events.every((e) => e.type === ADCSDK.EventType.DELETE)).toBe(true);
      await syncEvents(backend, events);
    });

    it('Dump again (all global rules removed)', async () => {
      const result = (await dumpConfiguration(backend)) as ADCSDK.Configuration;
      expect(Object.keys(result.global_rules!)).toHaveLength(0);
    });
  });
});
