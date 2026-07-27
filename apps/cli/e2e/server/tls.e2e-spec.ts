import { readFileSync } from 'node:fs';
import * as https from 'node:https';
import { join } from 'node:path';
import request from 'supertest';

import { ADCServer } from '../../src/server';

const readCert = (fileName: string) =>
  readFileSync(join(__dirname, '../assets/tls/', fileName), 'utf-8');

// a backend whose certificate is signed by a CA the system does not trust
const backendPort = 48570;
const backendURL = `https://localhost:${backendPort}`;

describe('Server - Backend TLS', () => {
  let server: ADCServer;
  let backend: https.Server;

  const syncTo = (opts: Record<string, unknown>) =>
    request(server.TEST_ONLY_getExpress())
      .put('/sync')
      .send({
        task: {
          opts: {
            backend: 'apisix',
            server: backendURL,
            token: 'mock',
            cacheKey: 'default',
            ...opts,
          },
          config: {},
        },
      });

  beforeAll(async () => {
    server = new ADCServer({
      listen: new URL('http://127.0.0.1:3000'),
      listenStatus: 3001,
    });
    backend = https.createServer(
      { cert: readCert('server.cer'), key: readCert('server.key') },
      (_, res) => (res.writeHead(404), res.end('{}')),
    );
    await new Promise<void>((resolve) =>
      backend.listen(backendPort, '127.0.0.1', resolve),
    );
  });

  afterAll(async () => {
    await new Promise<void>((resolve) => backend.close(() => resolve()));
  });

  it('rejects an untrusted certificate', async () => {
    const { status, body } = await syncTo({});

    expect(status).toEqual(500);
    expect(body.message).toMatch(/unable to verify the first certificate/);
  });

  it('accepts the certificate when its CA is provided', async () => {
    const { status, body } = await syncTo({ caCert: readCert('ca.cer') });

    // the TLS handshake succeeds, so the backend's HTTP error surfaces instead
    expect(status).toEqual(500);
    expect(body.message).toMatch(/status code 404/);
  });

  it('ignores the CA bundle when verification is off', async () => {
    const { status, body } = await syncTo({
      tlsSkipVerify: true,
      caCert: readCert('ca.cer'),
    });

    expect(status).toEqual(500);
    expect(body.message).toMatch(/status code 404/);
  });

  it('rejects a CA bundle that is not PEM encoded', async () => {
    const { status, body } = await syncTo({ caCert: 'not-a-certificate' });

    expect(status).toEqual(400);
    expect(body.message).toMatch(/caCert must be a PEM-encoded certificate/);
  });
});
