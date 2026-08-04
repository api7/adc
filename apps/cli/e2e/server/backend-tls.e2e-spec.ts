import { readFileSync } from 'node:fs';
import * as https from 'node:https';
import { join } from 'node:path';
import request from 'supertest';

import * as commandUtils from '../../src/command/utils';
import { ADCServer } from '../../src/server';
import { mockBackend } from '../support/utils';

const tlsAssetsDir = join(__dirname, '../assets/tls');
const readCert = (fileName: string) =>
  readFileSync(join(tlsAssetsDir, fileName), 'utf-8');

describe('Server - Backend TLS', () => {
  let server: ADCServer;

  beforeAll(() => {
    server = new ADCServer({
      listen: new URL('http://127.0.1:3000'),
      listenStatus: 3002,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('rejects a request with tlsClientCert but no tlsClientKey', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/sync')
      .send({
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
            tlsClientCert: readCert('client.cer'),
          },
          config: {},
        },
      });

    expect(status).toEqual(400);
    expect(
      (body.errors as Array<{ path: string[] }>).some((issue) =>
        issue.path.includes('tlsClientKey'),
      ),
    ).toBe(true);
  });

  it('rejects a caCert that does not look like PEM content', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/sync')
      .send({
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
            caCert: 'not-a-pem',
          },
          config: {},
        },
      });

    expect(status).toEqual(400);
    expect(
      (body.errors as Array<{ path: string[] }>).some((issue) =>
        issue.path.includes('caCert'),
      ),
    ).toBe(true);
  });

  it('reuses the same pooled agent across requests with identical TLS material, but not across different material', async () => {
    const loadBackendSpy = vi
      .spyOn(commandUtils, 'loadBackend')
      .mockImplementation(() => mockBackend());

    const sendSync = (caCert?: string) =>
      request(server.TEST_ONLY_getExpress())
        .put('/sync')
        .send({
          task: {
            opts: {
              backend: 'mock',
              server: 'http://1.1.1.1:3000',
              token: 'mock',
              cacheKey: 'default',
              ...(caCert ? { caCert } : {}),
            },
            config: {},
          },
        });

    const ca = readCert('ca.cer');
    await sendSync(ca);
    await sendSync(ca);
    await sendSync(); // no TLS material at all -> a different, insecure-default agent

    expect(loadBackendSpy).toHaveBeenCalledTimes(3);
    const httpsAgents = loadBackendSpy.mock.calls.map(
      ([, opts]) => (opts as { httpsAgent: unknown }).httpsAgent,
    );
    expect(httpsAgents[0]).toBeDefined();
    expect(httpsAgents[2]).toBeDefined();
    expect(httpsAgents[0]).toBe(httpsAgents[1]);
    expect(httpsAgents[0]).not.toBe(httpsAgents[2]);
  });

  it.each(['/sync', '/validate'] as const)(
    'does not forward raw TLS material to loadBackend for %s',
    async (route) => {
      const loadBackendSpy = vi
        .spyOn(commandUtils, 'loadBackend')
        .mockImplementation(() => mockBackend());

      await request(server.TEST_ONLY_getExpress())
        .put(route)
        .send({
          task: {
            opts: {
              backend: 'mock',
              server: 'http://1.1.1.1:3000',
              token: 'mock',
              cacheKey: 'default',
              caCert: readCert('ca.cer'),
              tlsClientCert: readCert('client.cer'),
              tlsClientKey: readCert('client.key'),
            },
            config: {},
          },
        });

      expect(loadBackendSpy).toHaveBeenCalledTimes(1);
      const [, opts] = loadBackendSpy.mock.calls[0];
      expect(opts).not.toHaveProperty('caCert');
      expect(opts).not.toHaveProperty('tlsClientCert');
      expect(opts).not.toHaveProperty('tlsClientKey');
      expect(opts).not.toHaveProperty('tlsSkipVerify');
      // secure default: no tlsSkipVerify means the pooled agent must still verify
      expect((opts as { httpsAgent: https.Agent }).httpsAgent.options.rejectUnauthorized).toBe(
        true,
      );
    },
  );

  describe('real backend connection', () => {
    let backendServer: https.Server;
    let backendPort: number;

    beforeAll(async () => {
      backendServer = https.createServer(
        { cert: readCert('server.cer'), key: readCert('server.key') },
        (_, res) => res.end('{}'),
      );
      await new Promise<void>((resolve) =>
        backendServer.listen(0, '127.0.0.1', resolve),
      );
      backendPort = (backendServer.address() as { port: number }).port;
    });

    afterAll(async () => {
      await new Promise<void>((resolve) => backendServer.close(() => resolve()));
    });

    it('fails with a certificate verification error when no caCert is provided', async () => {
      const { status, body } = await request(server.TEST_ONLY_getExpress())
        .put('/sync')
        .send({
          task: {
            opts: {
              backend: 'apisix',
              server: `https://127.0.0.1:${backendPort}`,
              token: 'mock',
              cacheKey: 'default',
            },
            config: {},
          },
        });

      expect(status).toEqual(500);
      expect(body.message).toMatch(/self-signed certificate|unable to verify/i);
    });

    it('does not fail on certificate verification once the signing caCert is provided', async () => {
      const { body } = await request(server.TEST_ONLY_getExpress())
        .put('/sync')
        .send({
          task: {
            opts: {
              backend: 'apisix',
              server: `https://127.0.0.1:${backendPort}`,
              token: 'mock',
              cacheKey: 'default',
              caCert: readCert('ca.cer'),
            },
            config: {},
          },
        });

      // the fake backend doesn't implement the real Admin API, so the request
      // may still fail (with any status) for unrelated reasons; the point of
      // this assertion is that it no longer fails on TLS certificate verification
      expect(body.message).not.toMatch(/self-signed certificate|unable to verify/i);
    });
  });
});
