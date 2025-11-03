import { globalAgent as httpAgent } from 'node:http';
import { globalAgent as httpsAgent } from 'node:https';

import { BackendAPI7 } from '../src';

describe('Ping', () => {
  it('should success', async () => {
    const backend = new BackendAPI7({
      server: process.env.SERVER!,
      token: process.env.TOKEN!,
      tlsSkipVerify: true,
      cacheKey: 'default',
      httpAgent,
      httpsAgent,
    });
    await backend.ping();
  });

  it('should failed (invalid server)', async () => {
    const backend = new BackendAPI7({
      server: 'http://0.0.0.0',
      token: '',
      cacheKey: 'default',
      httpAgent,
      httpsAgent,
    });
    await expect(backend.ping()).rejects.toThrow(
      'connect ECONNREFUSED 0.0.0.0:80',
    );
  });

  it('should failed (self-signed certificate)', async () => {
    const backend = new BackendAPI7({
      server: process.env.SERVER!,
      token: process.env.TOKEN!,
      cacheKey: 'default',
      httpAgent,
      httpsAgent,
    });
    await expect(backend.ping()).rejects.toThrow('self-signed certificate');
  });
});
