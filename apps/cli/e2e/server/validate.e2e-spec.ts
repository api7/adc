import * as ADCSDK from '@api7/adc-sdk';
import request from 'supertest';

import { ADCServer } from '../../src/server';
import { jestMockBackend } from '../support/utils';

describe('Server - Validate', () => {
  let server: ADCServer;

  beforeAll(async () => {
    jestMockBackend();
    server = new ADCServer({
      listen: new URL('http://127.0.1:3000'),
      listenStatus: 3001,
    });
  });

  it('test validate with empty config', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/validate')
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

    expect(status).toEqual(200);
    expect(body.success).toEqual(true);
    expect(body.source).toEqual('validate');
    expect(body.errors).toEqual([]);
  });

  it('test validate with config', async () => {
    const config = {
      consumers: [
        {
          username: 'test-consumer',
          plugins: { 'limit-count': { count: 10, time_window: 60 } },
        },
      ],
    } as ADCSDK.Configuration;
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/validate')
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

    expect(status).toEqual(200);
    expect(body.success).toEqual(true);
    expect(body.source).toEqual('validate');
    expect(body.errors).toEqual([]);
  });

  it('test validate with invalid input', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/validate')
      .send({
        task: {
          opts: {
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            cacheKey: 'default',
          },
          config: {},
        },
      });

    expect(status).toEqual(400);
    expect(body.success).toEqual(false);
    expect(body.source).toEqual('input');
  });

  it('test validate with lint failure', async () => {
    const { status, body } = await request(server.TEST_ONLY_getExpress())
      .put('/validate')
      .send({
        task: {
          opts: {
            backend: 'mock',
            server: 'http://1.1.1.1:3000',
            token: 'mock',
            lint: true,
            cacheKey: 'default',
          },
          config: {
            invalid_key: {},
          },
        },
      });

    expect(status).toEqual(400);
    expect(body.success).toEqual(false);
    expect(body.source).toEqual('lint');
  });
});
