import { redactRequestBody } from './logger';

describe('redactRequestBody', () => {
  it('redacts tlsClientKey while preserving other fields', () => {
    expect(
      redactRequestBody({
        task: {
          opts: { backend: 'apisix', tlsClientKey: 'SECRET', tlsClientCert: 'cert' },
          config: {},
        },
      }),
    ).toEqual({
      task: {
        opts: { backend: 'apisix', tlsClientKey: '***', tlsClientCert: 'cert' },
        config: {},
      },
    });
  });

  it('returns the body unchanged when tlsClientKey is absent', () => {
    const body = { task: { opts: { backend: 'apisix' }, config: {} } };
    expect(redactRequestBody(body)).toBe(body);
  });

  it.each([
    { task: { opts: 1 } },
    { task: { opts: 'not-an-object' } },
    { task: { opts: null } },
    { task: {} },
    {},
    undefined,
    null,
  ])('does not throw for malformed body %j', (body) => {
    expect(() => redactRequestBody(body)).not.toThrow();
  });
});
