import * as ADCSDK from '@api7/adc-sdk';
import { globalAgent as httpGlobalAgent } from 'node:http';
import { globalAgent as httpsGlobalAgent } from 'node:https';

export const server = 'http://localhost:19180';
export const token = 'edd1c9f034335f136f87ad84b625c8f1';

export const defaultBackendOptions: ADCSDK.BackendOptions = {
  server,
  token,
  cacheKey: 'default',
  httpAgent: httpGlobalAgent,
  httpsAgent: httpsGlobalAgent,
};
