import { defineConfig } from 'vitest/config';

// Isolated from vitest.config.ts's normal `**/*.spec.ts` test suite: this
// only runs the TS/Rust parity fixture dump (see tools/dump-fixture-results.ts),
// which has no assertions of its own — the comparison happens in
// scripts/compare-differ-fixtures.mjs after both sides have dumped their output.
export default defineConfig({
  cacheDir: '../../node_modules/.vitest/libs/differ-fixtures',
  test: {
    globals: true,
    environment: 'node',
    include: ['tools/*.ts'],
    reporters: ['default'],
  },
});
