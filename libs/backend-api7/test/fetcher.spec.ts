import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';
import { Subject } from 'rxjs';
import * as semver from 'semver';

import { Fetcher } from '../src/fetcher';

describe('Fetcher', () => {
  describe('API-level resource type filters', () => {
    type TrickFetcher = typeof Fetcher & {
      isSkip: Fetcher['isSkip'];
    };
    const subject = new Subject<ADCSDK.BackendEvent>();
    const newFetcher = (opts?: Partial<ADCSDK.BackendOptions>) =>
      new Fetcher({
        client: axios.create(),
        backendOpts: opts as ADCSDK.BackendOptions,
        version: semver.coerce('999.999.999'),
        eventSubject: subject,
      }) as unknown as TrickFetcher;

    it('should include services', () => {
      const fetcher = newFetcher({
        includeResourceType: [ADCSDK.ResourceType.SERVICE],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(true);
    });

    it('should include services, consumers, ssls', () => {
      const fetcher = newFetcher({
        includeResourceType: [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER,
          ADCSDK.ResourceType.SSL,
        ],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(true);
    });

    it('should include all', () => {
      const fetcher = newFetcher({
        includeResourceType: [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER,
          ADCSDK.ResourceType.SSL,
          ADCSDK.ResourceType.GLOBAL_RULE,
          ADCSDK.ResourceType.PLUGIN_METADATA,
        ],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(
        false,
      );
    });

    it('should include all (include list defined but empty)', () => {
      const fetcher = newFetcher({
        includeResourceType: [],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(
        false,
      );
    });

    it('should exclude services', () => {
      const fetcher = newFetcher({
        excludeResourceType: [ADCSDK.ResourceType.SERVICE],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(
        false,
      );
    });

    it('should exclude services, consumers, ssls', () => {
      const fetcher = newFetcher({
        excludeResourceType: [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER,
          ADCSDK.ResourceType.SSL,
        ],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(
        false,
      );
    });

    it('should exclude all', () => {
      const fetcher = newFetcher({
        excludeResourceType: [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER,
          ADCSDK.ResourceType.SSL,
          ADCSDK.ResourceType.GLOBAL_RULE,
          ADCSDK.ResourceType.PLUGIN_METADATA,
        ],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(true);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(true);
    });

    it('should include all (exclude list defined but empty)', () => {
      const fetcher = newFetcher({
        excludeResourceType: [],
      });
      expect(fetcher.isSkip(ADCSDK.ResourceType.SERVICE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.CONSUMER)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.SSL)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.GLOBAL_RULE)).toEqual(false);
      expect(fetcher.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA)).toEqual(
        false,
      );
    });
  });
});
