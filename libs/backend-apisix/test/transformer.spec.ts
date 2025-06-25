import { ToADC } from '../src/transformer';

describe('Transformer', () => {
  it('should transform upstream nodes to array', () => {
    const toADC = new ToADC();
    expect(
      toADC.transformUpstream({
        nodes: {
          '127.0.0.1:5432': 100,
        },
      }),
    ).toEqual({ nodes: [{ host: '127.0.0.1', port: 5432, weight: 100 }] });
  });
});
