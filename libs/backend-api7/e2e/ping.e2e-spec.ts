import { BackendAPI7 } from '../src';

describe('Ping', () => {
  it('should success', async () => {
    const backend = new BackendAPI7({
      server: globalThis.server,
      token: globalThis.token,
      tlsSkipVerify: true,
    });
    await backend.ping();
  });

  it('should failed (invalid server)', async () => {
    const backend = new BackendAPI7({
      server: 'http://0.0.0.0',
      token: '',
    });
    await expect(backend.ping()).rejects.toThrow(
      'connect ECONNREFUSED 0.0.0.0:80',
    );
  });

  it('should failed (self-signed certificate)', async () => {
    const backend = new BackendAPI7({
      server: globalThis.server,
      token: globalThis.token,
    });
    await expect(backend.ping()).rejects.toThrow('self-signed certificate');
  });
});
