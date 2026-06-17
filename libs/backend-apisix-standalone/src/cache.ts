import * as ADCSDK from '@api7/adc-sdk';
import { LRUCache } from 'lru-cache';
import { type SemVer } from 'semver';

import * as typing from './typing';

const parseEnvInt = (value: string | undefined, defaultVal: number): number => {
  const n = Number(value ?? defaultVal);
  return Number.isFinite(n) && n >= 1 ? Math.floor(n) : defaultVal;
};

const max = parseEnvInt(process.env.ADC_APISIX_STANDALONE_CACHE_MAX, 16);
const ttl = parseEnvInt(process.env.ADC_APISIX_STANDALONE_CACHE_TTL_MS, 3_600_000);

export const version = new LRUCache<string, SemVer>({ max, ttl });
export const latestVersion = new LRUCache<string, number>({ max, ttl });
export const config = new LRUCache<string, ADCSDK.Configuration>({ max, ttl });
export const rawConfig = new LRUCache<string, typing.APISIXStandalone>({ max, ttl });

export const invalidate = (key: string): void => {
  version.delete(key);
  latestVersion.delete(key);
  config.delete(key);
  rawConfig.delete(key);
};
