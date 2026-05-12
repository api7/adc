import * as ADCSDK from '@api7/adc-sdk';
import { Agent as httpAgent } from 'http';
import { Agent as httpsAgent } from 'https';
import { createServer } from 'node:http';
import { lastValueFrom, toArray } from 'rxjs';
import { AddressInfo } from 'node:net';

import { BackendAPI7 } from '../src';

describe('BackendAPI7 timeout error message', () => {
  // Create a local HTTP server that delays responses to trigger timeouts
  let server: ReturnType<typeof createServer>;
  let serverUrl: string;

  beforeAll(async () => {
    server = createServer((_, res) => {
      // Delay response by 5 seconds to guarantee timeout
      setTimeout(() => {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ value: {} }));
      }, 5000);
    });
    await new Promise<void>((resolve) => {
      server.listen(0, '127.0.0.1', () => resolve());
    });
    const addr = server.address() as AddressInfo;
    serverUrl = `http://127.0.0.1:${addr.port}`;
  });

  afterAll(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });

  const createBackend = (timeout: number) =>
    new BackendAPI7({
      server: serverUrl,
      token: 'test-token',
      gatewayGroup: 'default',
      timeout,
      cacheKey: 'test',
      httpAgent: new httpAgent({ keepAlive: false }),
      httpsAgent: new httpsAgent({ keepAlive: false }),
    });

  it('ping timeout should include request URL and timeout hint', async () => {
    const backend = createBackend(10);
    try {
      await backend.ping();
      throw new Error('Expected timeout error');
    } catch (err) {
      const error = err as Error;
      expect(error.message).toContain(
        `${serverUrl}/api/gateway_groups`,
      );
      expect(error.message).toContain('timed out after 10ms');
      expect(error.message).toContain('--timeout');
    }
  });

  it('version timeout should include request URL and timeout hint', async () => {
    const backend = createBackend(10);
    try {
      await backend.version();
      throw new Error('Expected timeout error');
    } catch (err) {
      const error = err as Error;
      expect(error.message).toContain(`${serverUrl}/api/version`);
      expect(error.message).toContain('timed out after 10ms');
      expect(error.message).toContain('--timeout');
    }
  });

  it('dump timeout should include request URL and timeout hint', async () => {
    const backend = createBackend(10);
    try {
      await lastValueFrom(backend.dump().pipe(toArray()));
      throw new Error('Expected timeout error');
    } catch (err) {
      const error = err as Error;
      expect(error.message).toContain(`${serverUrl}/api/`);
      expect(error.message).toContain('timed out after 10ms');
      expect(error.message).toContain('--timeout');
    }
  });

  it('sync timeout should include request URL and timeout hint', async () => {
    const backend = createBackend(10);
    try {
      await lastValueFrom(
        backend
          .sync(
            [
              {
                type: ADCSDK.EventType.CREATE,
                resourceType: ADCSDK.ResourceType.CONSUMER,
                resourceId: 'test-consumer',
                resourceName: 'test-consumer',
                newValue: {
                  username: 'test',
                  plugins: {},
                },
              },
            ],
            { exitOnFailure: true },
          )
          .pipe(toArray()),
      );
      throw new Error('Expected timeout error');
    } catch (err) {
      const error = err as Error;
      // sync first calls version() and getGatewayGroupId(), which will timeout
      expect(error.message).toContain(`${serverUrl}/api/`);
      expect(error.message).toContain('timed out after 10ms');
      expect(error.message).toContain('--timeout');
    }
  });
});
