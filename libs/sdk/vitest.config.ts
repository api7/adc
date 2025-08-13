import { defineConfig } from 'vitest/config';

export default defineConfig({
  cacheDir: '../../node_modules/.vitest/libs/sdk',
  test: {
    globals: true,
    environment: 'node',
    include: ['**/*.spec.ts'],
    reporters: ['default'],
    coverage: {
      reportsDirectory: '../../coverage/libs/sdk',
      provider: 'v8',
    },
  },
});
