import * as ADCSDK from '@api7/adc-sdk';
import { lastValueFrom } from 'rxjs';
import request from 'supertest';

import { ADCServer } from '../../src/server';
import { jestMockBackend } from '../support/utils';

describe('Server - Basic', () => {
  let mockedBackend: ADCSDK.Backend;
  let server: ADCServer;

  beforeAll(async () => {
    mockedBackend = jestMockBackend();
    server = new ADCServer();
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
          },
          config: {},
        },
      });

    expect(status).toBe(202);
    expect(body.status).toBe('success');
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
          },
          config: {},
        },
      });

    expect(status).toBe(500);
    expect(body.message).toBe('Error: connect ECONNREFUSED 127.0.0.1:50000');
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
          },
          config: config,
        },
      });

    expect(status).toBe(202);
    expect(body.status).toBe('success');
    await expect(lastValueFrom(mockedBackend.dump())).resolves.toEqual(config);
  });
});
