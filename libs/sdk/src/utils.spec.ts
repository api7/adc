import { utils } from './utils';

describe('SDK utils', () => {
  it('recursiveOmitUndefined', () => {
    expect(
      utils.recursiveOmitUndefined({
        test: 'test',
        removed: undefined,
        test2: {
          test3: 'test',
          removed: undefined,
        },
        test5: ['test', undefined],
      }),
    ).toEqual({
      test: 'test',
      test2: { test3: 'test' },
      test5: ['test', undefined],
    });
  });
});
