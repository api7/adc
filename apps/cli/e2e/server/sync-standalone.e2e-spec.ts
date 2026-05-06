import { BackendAPISIXStandalone } from '@api7/adc-backend-apisix-standalone';
import * as ADCSDK from '@api7/adc-sdk';

import { ADCServer } from '../../src/server';

describe('Server - Sync (Standalone)', () => {
  let backend: ADCSDK.Backend;
  let server: ADCServer;

  beforeAll(async () => {
    backend = new BackendAPISIXStandalone();
    server = new ADCServer({
      listen: new URL('http://127.0.1:3000'),
      listenStatus: 3001,
    });
  });
});
