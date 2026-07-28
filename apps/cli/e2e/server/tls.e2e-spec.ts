import { readFileSync } from 'node:fs';
import * as https from 'node:https';
import { join } from 'node:path';
import request from 'supertest';

import { ADCServer } from '../../src/server';

const readCert = (fileName: string) =>
  readFileSync(join(__dirname, '../assets/tls/', fileName), 'utf-8');

describe('Server - Backend TLS', () => {
  let server: ADCServer;
  // a backend whose certificate is signed by a CA the system does not trust
  let backend: https.Server;
  let backendURL: string;

  const send = (path: string, opts: Record<string, unknown>) =>
    request(server.TEST_ONLY_getExpress())
      .put(path)
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
    await new Promise<void>((resolve, reject) => {
      const onError = (err: Error) => reject(err);
      backend.once('error', onError);
      backend.listen(0, '127.0.0.1', () => {
        backend.off('error', onError);
        const address = backend.address();
        if (!address || typeof address === 'string')
          return reject(new Error('backend did not bind a TCP port'));
        // the test certificate is issued for localhost
        backendURL = `https://localhost:${address.port}`;
        resolve();
      });
    });
  });

  afterAll(async () => {
    await new Promise<void>((resolve) => backend.close(() => resolve()));
  });

  // once the handshake succeeds the request reaches the backend, and each
  // endpoint fails on what it finds there instead of on the certificate
  describe.each([
    { path: '/sync', reachedBackend: /status code 404/ },
    { path: '/validate', reachedBackend: /Validate is not supported/ },
  ])('$path', ({ path, reachedBackend }) => {
    it('rejects an untrusted certificate', async () => {
      const { body } = await send(path, {});

      expect(body.message).toMatch(/unable to verify the first certificate/);
    });

    it('accepts the certificate when its CA is provided', async () => {
      const { body } = await send(path, { caCert: readCert('ca.cer') });

      expect(body.message).toMatch(reachedBackend);
    });

    it('ignores the CA bundle when verification is off', async () => {
      const { body } = await send(path, {
        tlsSkipVerify: true,
        caCert: readCert('ca.cer'),
      });

      expect(body.message).toMatch(reachedBackend);
    });

    it.each([
      ['not PEM at all', 'not-a-certificate'],
      ['a header with no certificate', '-----BEGIN CERTIFICATE-----'],
      [
        'an unparseable body',
        '-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----',
      ],
      [
        'one good and one broken certificate',
        `${readCert('ca.cer')}\n-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----`,
      ],
    ])('rejects a CA bundle that is %s', async (_, caCert) => {
      const { status, body } = await send(path, { caCert });

      expect(status).toEqual(400);
      expect(body.message).toMatch(/caCert must be a PEM-encoded certificate/);
    });
  });
});
