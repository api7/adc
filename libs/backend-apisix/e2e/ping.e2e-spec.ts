import { readFileSync } from 'node:fs';
import { globalAgent as httpAgent } from 'node:http';
import {
  Agent as httpsAgent,
  globalAgent as httpsGlobalAgent,
} from 'node:https';
import { join } from 'path';

import { BackendAPISIX } from '../src';
import { server, token } from './support/constants';

describe('Ping', () => {
  it('should success (http)', async () => {
    const backend = new BackendAPISIX({
      server,
      token,
      cacheKey: 'default',
      httpAgent,
      httpsAgent: httpsGlobalAgent,
    });
    await backend.ping();
  });

  it('should success (mTLS)', async () => {
    const backend = new BackendAPISIX({
      server: 'https://localhost:29180',
      token,
      cacheKey: 'default',
      httpAgent,
      httpsAgent: new httpsAgent({
        cert: readFileSync(
          join(__dirname, 'assets/apisix_conf/mtls/client.cer'),
        ),
        key: readFileSync(
          join(__dirname, 'assets/apisix_conf/mtls/client.key'),
        ),
        ca: readFileSync(join(__dirname, 'assets/apisix_conf/mtls/ca.cer')),
      }),
      tlsClientCertFile: join(__dirname, 'assets/apisix_conf/mtls/client.cer'),
      tlsClientKeyFile: join(__dirname, 'assets/apisix_conf/mtls/client.key'),
      caCertFile: join(__dirname, 'assets/apisix_conf/mtls/ca.cer'),
    });
    await backend.ping();
  });

  it('should failed (invalid server)', async () => {
    const backend = new BackendAPISIX({
      server: 'http://0.0.0.0',
      token: '',
      cacheKey: 'default',
      httpAgent,
      httpsAgent: httpsGlobalAgent,
    });
    await expect(backend.ping()).rejects.toThrow(
      'connect ECONNREFUSED 0.0.0.0:80',
    );
  });

  it('should failed (self-signed certificate)', async () => {
    const backend = new BackendAPISIX({
      server: 'https://localhost:29180',
      token,
      cacheKey: 'default',
      httpAgent,
      httpsAgent: httpsGlobalAgent,
    });
    await expect(backend.ping()).rejects.toThrow(
      'unable to verify the first certificate',
    );
  });

  it('should failed (miss client certificates)', async () => {
    const backend = new BackendAPISIX({
      server: 'https://localhost:29180',
      token,
      cacheKey: 'default',
      httpAgent,
      httpsAgent: new httpsAgent({
        cert: readFileSync(
          join(__dirname, 'assets/apisix_conf/mtls/client.cer'),
        ),
      }),
      caCertFile: join(__dirname, 'assets/apisix_conf/mtls/ca.cer'),
    });

    try {
      await backend.ping();
    } catch (err) {
      expect(err.toString()).toContain('Request failed with status code 400');
      expect(err.response.data).toContain(
        'No required SSL certificate was sent',
      );
    }
  });
});
