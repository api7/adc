/* eslint-disable */
export default {
  displayName: 'converter-openapi',
  preset: '../../jest.preset.js',
  testEnvironment: 'node',
  transform: {
    '^.+\\.[tj]s$': ['ts-jest', { tsconfig: '<rootDir>/tsconfig.spec.json' }],
  },
  moduleFileExtensions: ['ts', 'js', 'html'],
  coverageDirectory: '../../coverage/libs/converter-openapi',
  testMatch: ['**/test/?(*.)+(spec).ts?(x)'],
};
