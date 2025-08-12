import { defineConfig } from 'vitest/config';

export default defineConfig({
  cacheDir: '../node_modules/.vitest/<project-root>',
  test: {
    globals: true,
    environment: 'node',
    include: ['**/*.spec.ts'],
    reporters: ['default'],
    coverage: {
      reportsDirectory: '../coverage/<project-root>',
      provider: 'v8',
    },
    poolOptions: { forks: { singleFork: true } },
    retry: 0,
  },
});
