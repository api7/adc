import { globalAgent as httpGlobalAgent } from 'node:http';
import { globalAgent as httpsGlobalAgent } from 'node:https';

export const server1 = 'http://localhost:19180';
export const token1 = 'edd1c9f034335f136f87ad84b625c8f1';

export const server2 = 'http://localhost:29180';
export const token2 = 'edd1c9f034335f136f87ad84b625c8f1';

export const server3 = 'http://localhost:39180';
export const token3 = 'edd1c9f034335f136f87ad84b625c8f1';

export const servers =
  'http://localhost:19180,http://localhost:29180,http://localhost:39180';
export const tokens =
  'edd1c9f034335f136f87ad84b625c8f1,edd1c9f034335f136f87ad84b625c8f1,edd1c9f034335f136f87ad84b625c8f1';
export const defaultBackendOptions = {
  httpAgent: httpGlobalAgent,
  httpsAgent: httpsGlobalAgent,
};
