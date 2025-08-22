import * as ADCSDK from '@api7/adc-sdk';
import pluralize from 'pluralize';
import { Subscription, from, of, switchMap, tap, toArray } from 'rxjs';
import { SemVer } from 'semver';

import * as commandUtils from '../../src/command/utils';

export const mockBackend = (): ADCSDK.Backend => {
  class MockBackend implements ADCSDK.Backend {
    private readonly cache: { config?: ADCSDK.Configuration } = {};

    public metadata() {
      return { logScope: ['mock'] };
    }
    public ping() {
      return Promise.resolve();
    }
    public version() {
      return Promise.resolve(new SemVer('0.0.0-mock'));
    }
    public defaultValue() {
      return Promise.resolve({});
    }
    public dump() {
      return of(this.cache.config);
    }
    public sync(events: Array<ADCSDK.Event>) {
      const config: ADCSDK.Configuration = {};
      const resourceTypeToPluralKey = (type: ADCSDK.ResourceType) =>
        pluralize.plural(type);
      return from(events).pipe(
        tap((event) => {
          if (event.type === ADCSDK.EventType.DELETE) return;
          const key = resourceTypeToPluralKey(event.resourceType);
          if (!config[key]) config[key] = [];
          config[key].push(event.newValue);
        }),
        toArray(),
        switchMap(
          () => (
            (this.cache.config = config),
            of({
              success: true,
              event: {} as ADCSDK.Event, // keep empty
            } as ADCSDK.BackendSyncResult)
          ),
        ),
      );
    }
    public on() {
      return new Subscription();
    }
    supportValidate?: () => Promise<boolean>;
    supportStreamRoute?: () => Promise<boolean>;
  }
  return new MockBackend();
};

export const jestMockBackend = (mockedBackend?: ADCSDK.Backend) => {
  if (!mockedBackend) mockedBackend = mockBackend();
  const originalLoadBackend = commandUtils.loadBackend;
  vi.spyOn(commandUtils, 'loadBackend').mockImplementation((backend, opts) => {
    if (backend === 'mock') return mockedBackend;
    return originalLoadBackend(backend, opts);
  });
  return mockedBackend;
};
