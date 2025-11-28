import * as ADCSDK from '@api7/adc-sdk';
import { type SemVer } from 'semver';

import * as typing from './typing';

export const startTime = Date.now();
export const version = new Map<string, SemVer>();
export const latestVersion = new Map<string, number>();
export const config = new Map<string, ADCSDK.Configuration>();
export const rawConfig = new Map<string, typing.APISIXStandalone>();
