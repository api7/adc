import { config } from 'typescript-eslint';

import baseConfig from '../../eslint.config.js';

export default config([
  ...baseConfig,
  {
    files: ['**/*.json'],
    rules: {
      '@nx/dependency-checks': [
        'error',
        {
          ignoredFiles: [
            '{projectRoot}/eslint.config.{js,cjs,mjs,ts,cts,mts}',
            '{projectRoot}/vitest.config.{js,ts,mjs,mts}',
            '{projectRoot}/test/**/*',
            '{projectRoot}/e2e/**/*',
          ],
          ignoredDependencies: [
            'tslib',
            // lru-cache v11 does not expose ./package.json via its exports map, and its
            // dist/commonjs/package.json lacks name/version fields. Nx's package resolver
            // hits that incomplete package.json and returns null instead of walking up to
            // the real one, so the import in cache.ts is never recorded as an npm dep.
            'lru-cache',
          ],
        },
      ],
    },
    languageOptions: {
      parser: await import('jsonc-eslint-parser'),
    },
  },
]);
