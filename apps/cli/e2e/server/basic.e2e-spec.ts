import * as ADCSDK from '@api7/adc-sdk';
import axios from 'axios';
import { readFileSync } from 'node:fs';
import * as http from 'node:http';
import * as https from 'node:https';
import { join } from 'node:path';
import { lastValueFrom } from 'rxjs';
import request from 'supertest';

import { ADCServer } from '../../src/server';
import { jestMockBackend } from '../support/utils';

describe('Server - Basic', () => {
  let mockedBackend: ADCSDK.Backend;
  let server: ADCServer;

  beforeAll(async () => {
    mockedBackend = jestMockBackend();
    server = new ADCServer({
      listen: new URL('http://127.0.1:3000'),
      listenStatus: 3001,
    });
  });

  it('test mocked load backend', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/sync')
      .send({
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
          },
          config: {},
        },
      });

    expect(status).toEqual(202);
    expect(body.status).toEqual('success');
    await expect(lastValueFrom(mockedBackend.dump())).resolves.toEqual({});
  });

  it('test real apisix backend (expect connect refused)', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/sync')
      .send({
        task: {
          opts: {
            backend: 'apisix',
            server: 'http://127.0.0.1:50000',
            token: 'mock',
            cacheKey: 'default',
          },
          config: {},
        },
      });

    expect(status).toEqual(500);
    expect(body.message).toEqual('Error: connect ECONNREFUSED 127.0.0.1:50000');
  });

  it('test sync', async () => {
    const config = {
      consumers: [
        {
          username: 'test-consumer',
          plugins: { 'limit-count': { count: 10, time_window: 60 } },
        },
      ],
    } as ADCSDK.Configuration;
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/sync')
      .send({
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
          },
          config,
        },
      });

    expect(status).toEqual(202);
    expect(body.status).toEqual('success');
    await expect(lastValueFrom(mockedBackend.dump())).resolves.toEqual(config);
  });

  it('test server listen', async () => {
    const url = new URL(`http://127.0.0.1:48562`);
    const server = new ADCServer({ listen: url, listenStatus: 3001 });
    await server.start();

    const { status, data } = await axios.put(`${url.origin}/sync`, {
      task: {
        opts: {
          backend: 'mock',
          server: 'http://1.1.1.1:3000',
          token: 'mock',
          cacheKey: 'default',
        },
        config: {},
      },
    });
    expect(status).toEqual(202);
    expect(data.status).toEqual('success');

    await server.stop();
  });

  it('test server listen (with HTTPS, self-signed)', async () => {
    const url = new URL(`https://127.0.0.1:48562`);
    const server = new ADCServer({
      listen: url,
      listenStatus: 3001,
      tlsCert: readFileSync(
        join(__dirname, '../assets/tls/server.cer'),
        'utf-8',
      ),
      tlsKey: readFileSync(
        join(__dirname, '../assets/tls/server.key'),
        'utf-8',
      ),
    });
    await server.start();

    const { status, data } = await axios.put(
      `${url.origin}/sync`,
      {
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
          },
          config: {},
        },
      },
      { httpsAgent: new https.Agent({ rejectUnauthorized: false }) },
    );
    expect(status).toEqual(202);
    expect(data.status).toEqual('success');

    await server.stop();
  });

  it('test server listen (with HTTPS-mTLS, self-signed)', async () => {
    const url = new URL(`https://127.0.0.1:48562`);
    const readCert = (fileName: string) =>
      readFileSync(join(__dirname, '../assets/tls/', fileName), 'utf-8');
    const server = new ADCServer({
      listen: url,
      listenStatus: 3001,
      tlsCert: readCert('server.cer'),
      tlsKey: readCert('server.key'),
      tlsCACert: readCert('ca.cer'),
    });
    await server.start();

    // without client cert/key, should fail
    await expect(
      axios.put(
        `${url.origin}/sync`,
        {
          task: {
            opts: {
              backend: 'mock',
              server: 'http://1.1.1.1:3000',
              token: 'mock',
              cacheKey: 'default',
            },
            config: {},
          },
        },
        { httpsAgent: new https.Agent({ rejectUnauthorized: false }) },
      ),
    ).rejects.toThrow(/certificate required/);

    // with client cert/key, should succeed
    const { status, data } = await axios.put(
      `${url.origin}/sync`,
      {
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
          },
          config: {},
        },
      },
      {
        httpsAgent: new https.Agent({
          rejectUnauthorized: false,
          cert: readCert('client.cer'),
          key: readCert('client.key'),
        }),
      },
    );
    expect(status).toEqual(202);
    expect(data.status).toEqual('success');

    await server.stop();
  });

  it('test server listen (with UDS)', async () => {
    const url = new URL(`unix:///tmp/adc-test.sock`);
    const server = new ADCServer({ listen: url, listenStatus: 3001 });
    await server.start();

    const { status, data } = await new Promise<{
      status: number;
      data: { status: string };
    }>((resolve, reject) => {
      const req = http.request(
        'http://localhost/sync',
        {
          socketPath: url.pathname,
          method: 'PUT',
          headers: {
            'Content-Type': 'application/json',
          },
        },
        (res) => {
          let body = '';
          res.setEncoding('utf-8');
          res.on('data', (chunk) => {
            body += chunk;
          });
          res.on('end', () => {
            expect(res.statusCode).toBe(202);
            expect(JSON.parse(body).status).toBe('success');
            resolve({
              status: res.statusCode,
              data: JSON.parse(body),
            });
          });
        },
      );
      req.on('error', reject);
      req.write(
        JSON.stringify({
          task: {
            opts: {
              backend: 'mock',
              server: 'http://1.1.1.1:3000',
              token: 'mock',
              cacheKey: 'default',
            },
            config: {},
          },
        }),
      );
      req.end();
    });
    expect(status).toEqual(202);
    expect(data.status).toEqual('success');

    await server.stop();
  });

  it('test status listen', async () => {
    const server = new ADCServer({
      listen: new URL(`http://127.0.0.1:3000`),
      listenStatus: 3001,
    });
    await server.start();

    const { status, data } = await axios.get(
      `http://127.0.0.1:3001/healthz/ready`,
    );
    expect(status).toEqual(200);
    expect(data).toEqual('OK');

    await server.stop();
  });

  it('test status listen (custom port)', async () => {
    const server = new ADCServer({
      listen: new URL(`http://127.0.0.1:3000`),
      listenStatus: 30001,
    });
    await server.start();

    const { status, data } = await axios.get(
      `http://127.0.0.1:30001/healthz/ready`,
    );
    expect(status).toEqual(200);
    expect(data).toEqual('OK');

    await server.stop();
  });
});
