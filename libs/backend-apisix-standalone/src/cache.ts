import * as ADCSDK from '@api7/adc-sdk';
import { type SemVer } from 'semver';

import * as typing from './typing';

export const version = new Map<string, SemVer>();
export const config = new Map<string, ADCSDK.Configuration>();
export const rawConfig = new Map<
  string,
  typing.APISIXStandaloneWithConfVersionType
>();
