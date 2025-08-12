import { defineConfig } from 'vitest/config';

const isE2E = process.env.E2E === '1';

export default defineConfig({
  cacheDir: '../node_modules/.vitest/<project-root>',
  test: {
    globals: true,
    environment: 'node',
    include: [isE2E ? '**/*.e2e-spec.ts' : '**/*.spec.ts'],
    reporters: ['default'],
    coverage: {
      reportsDirectory: '../coverage/<project-root>',
      provider: 'v8',
    },
  },
});
