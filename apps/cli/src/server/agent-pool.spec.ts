import { HttpsAgent } from 'agentkeepalive';
import * as https from 'node:https';
import { join } from 'node:path';
import { readFileSync } from 'node:fs';

import {
  fingerprintTlsMaterial,
  getHttpsAgent,
  type TlsMaterial,
} from './agent-pool';

const tlsAssetsDir = join(__dirname, '../../e2e/assets/tls');
const readAsset = (fileName: string) =>
  readFileSync(join(tlsAssetsDir, fileName), 'utf-8');

describe('agent-pool fingerprintTlsMaterial', () => {
  it('produces the same fingerprint for identical TLS material', () => {
    const tls: TlsMaterial = { caCert: 'ca-content', tlsSkipVerify: true };
    expect(fingerprintTlsMaterial(tls)).toEqual(
      fingerprintTlsMaterial({ ...tls }),
    );
  });

  it('treats a missing tlsSkipVerify the same as an explicit false', () => {
    expect(fingerprintTlsMaterial({ caCert: 'ca-content' })).toEqual(
      fingerprintTlsMaterial({ caCert: 'ca-content', tlsSkipVerify: false }),
    );
  });

  it('produces different fingerprints when any field differs', () => {
    const base = fingerprintTlsMaterial({ caCert: 'ca-content' });
    expect(fingerprintTlsMaterial({ caCert: 'other-content' })).not.toEqual(
      base,
    );
    expect(fingerprintTlsMaterial({ tlsSkipVerify: true })).not.toEqual(base);
    expect(
      fingerprintTlsMaterial({
        caCert: 'ca-content',
        tlsClientCert: 'cert',
      }),
    ).not.toEqual(base);
  });
});

describe('agent-pool getHttpsAgent pooling', () => {
  it('reuses the same agent instance for identical TLS material', () => {
    const agent1 = getHttpsAgent({ caCert: 'shared-ca' });
    const agent2 = getHttpsAgent({ caCert: 'shared-ca' });
    expect(agent1).toBe(agent2);
  });

  it('returns isolated agent instances for different TLS material', () => {
    const agent1 = getHttpsAgent({ caCert: 'ca-a' });
    const agent2 = getHttpsAgent({ caCert: 'ca-b' });
    expect(agent1).not.toBe(agent2);
  });

  it('builds an agent with the requested TLS options', () => {
    const agent = getHttpsAgent({ tlsSkipVerify: true, caCert: 'ca-c' });
    expect(agent).toBeInstanceOf(HttpsAgent);
    expect(agent.options.rejectUnauthorized).toBe(false);
    expect(agent.options.ca).toEqual('ca-c');
  });

  it('defaults to rejectUnauthorized: true when no TLS material is given', () => {
    const agent = getHttpsAgent();
    expect(agent.options.rejectUnauthorized).toBe(true);
  });
});

describe('agent-pool getHttpsAgent real TLS handshake', () => {
  let server: https.Server;
  let port: number;

  beforeAll(async () => {
    server = https.createServer(
      {
        cert: readAsset('server.cer'),
        key: readAsset('server.key'),
      },
      (_, res) => res.end('ok'),
    );
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    port = (server.address() as { port: number }).port;
  });

  afterAll(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });

  // server.cer's CN is "localhost" (no IP SAN), so pin SNI/hostname
  // verification to "localhost" while still dialing the loopback IP directly
  const request = (agent: https.Agent) =>
    new Promise<void>((resolve, reject) => {
      https
        .get(
          { hostname: '127.0.0.1', servername: 'localhost', port, path: '/', agent },
          (res) => {
            res.resume();
            res.on('end', resolve);
          },
        )
        .on('error', reject);
    });

  it('connects successfully when trusting the signing CA', async () => {
    const agent = getHttpsAgent({ caCert: readAsset('ca.cer') });
    await expect(request(agent)).resolves.toBeUndefined();
  });

  it('fails certificate verification without the CA', async () => {
    const agent = getHttpsAgent();
    await expect(request(agent)).rejects.toThrow(/self.signed|unable to verify/i);
  });
});

describe('agent-pool LRU eviction', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubEnv('ADC_INGRESS_TLS_AGENT_POOL_MAX', '2');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('destroys the least-recently-used agent once the pool exceeds its max size', async () => {
    const pool = await import('./agent-pool');

    const agentA = pool.getHttpsAgent({ caCert: 'a' });
    const destroySpy = vi.spyOn(agentA, 'destroy');
    pool.getHttpsAgent({ caCert: 'b' });
    // exceeding max size (2) evicts the least-recently-used entry (a)
    pool.getHttpsAgent({ caCert: 'c' });

    expect(destroySpy).toHaveBeenCalledTimes(1);
  });
});
