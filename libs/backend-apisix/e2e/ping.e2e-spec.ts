import { join } from 'path';

import { BackendAPISIX } from '../src';
import { server, token } from './support/constants';

describe('Ping', () => {
  it('should success (http)', async () => {
    const backend = new BackendAPISIX({
      server,
      token,
    });
    await backend.ping();
  });

  it('should success (mTLS)', async () => {
    const backend = new BackendAPISIX({
      server: 'https://localhost:29180',
      token,
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
    });
    await expect(backend.ping()).rejects.toThrow(
      'connect ECONNREFUSED 0.0.0.0:80',
    );
  });

  it('should failed (self-signed certificate)', async () => {
    const backend = new BackendAPISIX({
      server: 'https://localhost:29180',
      token,
    });
    await expect(backend.ping()).rejects.toThrow(
      'unable to verify the first certificate',
    );
  });

  it('should failed (miss client certificates)', async () => {
    const backend = new BackendAPISIX({
      server: 'https://localhost:29180',
      token,
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
