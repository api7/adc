import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';

import { Fetcher } from '../src/fetcher';

describe('Fetcher', () => {
  describe('API-level resource type filters', () => {
    type TrickFetcher = typeof Fetcher & {
      isSkip: Fetcher['isSkip'];
    };
    const newFetcher = (opts?: Partial<ADCSDK.BackendOptions>) =>
      new Fetcher(
        axios.create(),
        opts as ADCSDK.BackendOptions,
      ) as unknown as TrickFetcher;
    const checkSkip = (func: () => string | undefined) => !!func();

    it('should include services', () => {
      const fetcher = newFetcher({
        includeResourceType: [ADCSDK.ResourceType.SERVICE],
      });
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        true,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        true,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(true);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(true);
    });

    it('should include services, consumers, ssls', () => {
      const fetcher = newFetcher({
        includeResourceType: [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER,
          ADCSDK.ResourceType.SSL,
        ],
      });
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        false,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(true);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(true);
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
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        false,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(false);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(false);
    });

    it('should include all (include list defined but empty)', () => {
      const fetcher = newFetcher({
        includeResourceType: [],
      });
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        false,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(false);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(false);
    });

    it('should exclude services', () => {
      const fetcher = newFetcher({
        excludeResourceType: [ADCSDK.ResourceType.SERVICE],
      });
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        true,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        false,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(false);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(false);
    });

    it('should exclude services, consumers, ssls', () => {
      const fetcher = newFetcher({
        excludeResourceType: [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER,
          ADCSDK.ResourceType.SSL,
        ],
      });
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        true,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        true,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        true,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(false);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(false);
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
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        true,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        true,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        true,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(true);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(true);
    });

    it('should include all (exclude list defined but empty)', () => {
      const fetcher = newFetcher({
        excludeResourceType: [],
      });
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SERVICE]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.CONSUMER]))).toEqual(
        false,
      );
      expect(checkSkip(fetcher.isSkip([ADCSDK.ResourceType.SSL]))).toEqual(
        false,
      );
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.GLOBAL_RULE])),
      ).toEqual(false);
      expect(
        checkSkip(fetcher.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA])),
      ).toEqual(false);
    });
  });
});
