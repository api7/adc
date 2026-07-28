import { MAX_CA_CERT_AGENTS, resolveHttpsAgent } from './agents';

const CA_A = `-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----`;
const CA_B = `-----BEGIN CERTIFICATE-----\nBBBB\n-----END CERTIFICATE-----`;

describe('Server - HTTPS agents', () => {
  it('verifies against the system trust store by default', () => {
    const agent = resolveHttpsAgent({});
    expect(agent.options.rejectUnauthorized).toEqual(true);
    expect(agent.options.ca).toBeUndefined();
  });

  it('skips verification when tlsSkipVerify is set', () => {
    const agent = resolveHttpsAgent({ tlsSkipVerify: true });
    expect(agent.options.rejectUnauthorized).toEqual(false);
  });

  it('verifies against the given CA bundle', () => {
    const agent = resolveHttpsAgent({ caCert: CA_A });
    expect(agent.options.rejectUnauthorized).toEqual(true);
    expect(agent.options.ca).toEqual(CA_A);
  });

  it('reuses one agent per CA bundle, so sockets stay pooled', () => {
    expect(resolveHttpsAgent({ caCert: CA_A })).toBe(
      resolveHttpsAgent({ caCert: CA_A }),
    );
    expect(resolveHttpsAgent({ caCert: CA_A })).not.toBe(
      resolveHttpsAgent({ caCert: CA_B }),
    );
    expect(resolveHttpsAgent({ caCert: CA_A })).not.toBe(resolveHttpsAgent({}));
  });

  it('reuses one agent across equivalent bundles', () => {
    expect(resolveHttpsAgent({ caCert: `  ${CA_A}\n` })).toBe(
      resolveHttpsAgent({ caCert: CA_A }),
    );
    expect(resolveHttpsAgent({ caCert: '   ' })).toBe(resolveHttpsAgent({}));
  });

  it('caps the cache, evicting the least recently used bundle', () => {
    const bundle = (i: number) =>
      `-----BEGIN CERTIFICATE-----\ncap-${i}\n-----END CERTIFICATE-----`;

    const first = resolveHttpsAgent({ caCert: bundle(0) });
    const second = resolveHttpsAgent({ caCert: bundle(1) });

    // fill the cache, keeping the first bundle in use so the second ages out
    for (let i = 2; i < MAX_CA_CERT_AGENTS + 2; i++) {
      resolveHttpsAgent({ caCert: bundle(i) });
      resolveHttpsAgent({ caCert: bundle(0) });
    }

    expect(resolveHttpsAgent({ caCert: bundle(0) })).toBe(first);
    expect(resolveHttpsAgent({ caCert: bundle(1) })).not.toBe(second);
  });

  it('tlsSkipVerify takes precedence over the CA bundle', () => {
    const agent = resolveHttpsAgent({ tlsSkipVerify: true, caCert: CA_A });
    expect(agent.options.rejectUnauthorized).toEqual(false);
    expect(agent.options.ca).toBeUndefined();
  });
});
