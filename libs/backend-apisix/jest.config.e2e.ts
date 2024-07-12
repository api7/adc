/* eslint-disable */
export default {
  displayName: 'backend-apisix-e2e',
  preset: '../../jest.preset.js',
  globalSetup: '<rootDir>/e2e/support/global-setup.ts',
  globalTeardown: '<rootDir>/e2e/support/global-teardown.ts',
  testEnvironment: 'node',
  transform: {
    '^.+\\.[tj]s$': [
      'ts-jest',
      {
        tsconfig: '<rootDir>/tsconfig.spec.json',
      },
    ],
  },
  moduleFileExtensions: ['ts', 'js', 'html'],
  coverageDirectory: '../../coverage/libs/backend-apisix/e2e',
  testMatch: ['**/?(*.)+(e2e-spec).[jt]s?(x)'],
};
