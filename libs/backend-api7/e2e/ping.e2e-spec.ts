import { BackendAPI7 } from '../src';

describe('Ping', () => {
  it('should success', async () => {
    const backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
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
      server: process.env.SERVER,
      token: process.env.TOKEN,
    });
    await expect(backend.ping()).rejects.toThrow('self-signed certificate');
  });
});
