import { defineConfig } from 'vitest/config';

const isE2E = process.env.E2E === '1';

export default defineConfig({
  cacheDir: '../../node_modules/.vitest/libs/backend-apisix-standalone',
  test: {
    globals: true,
    environment: 'node',
    include: [isE2E ? 'e2e/**/*.e2e-spec.ts' : 'test/**/*.spec.ts'],
    reporters: ['default'],
    coverage: {
      reportsDirectory: isE2E
        ? '../../coverage/libs/backend-apisix-standalone/e2e'
        : '../../coverage/libs/backend-apisix-standalone',
      provider: 'v8',
    },
    maxWorkers: 1,
    isolate: false,
  },
});
