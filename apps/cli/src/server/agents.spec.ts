import { resolveHttpsAgent } from './agents';

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

  it('tlsSkipVerify takes precedence over the CA bundle', () => {
    const agent = resolveHttpsAgent({ tlsSkipVerify: true, caCert: CA_A });
    expect(agent.options.rejectUnauthorized).toEqual(false);
    expect(agent.options.ca).toBeUndefined();
  });
});
