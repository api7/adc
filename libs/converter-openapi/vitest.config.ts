import { defineConfig } from 'vitest/config';

export default defineConfig({
  cacheDir: '../../node_modules/.vitest/libs/converter-openapi',
  test: {
    globals: true,
    environment: 'node',
    include: ['**/*.spec.ts'],
    reporters: ['default'],
    coverage: {
      reportsDirectory: '../../coverage/libs/converter-openapi',
      provider: 'v8',
    },
  },
});
