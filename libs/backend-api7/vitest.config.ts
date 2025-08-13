import { defineConfig } from 'vitest/config';

const isE2E = process.env.E2E === '1';

export default defineConfig({
  cacheDir: '../../node_modules/.vitest/libs/backend-api7',
  test: {
    globals: true,
    environment: 'node',
    include: [isE2E ? 'e2e/**/*.e2e-spec.ts' : 'test/**/*.spec.ts'],
    reporters: ['default'],
    coverage: {
      reportsDirectory: isE2E
        ? '../../coverage/libs/backend-api7/e2e'
        : '../../coverage/libs/backend-api7',
      provider: 'v8',
    },
    poolOptions: { forks: { singleFork: true } },
    ...(isE2E ? { globalSetup: '<rootDir>/e2e/support/global-setup.ts' } : {}),
  },
});
