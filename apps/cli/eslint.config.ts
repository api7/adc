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
            '{projectRoot}/e2e/**/*',
          ],
          // false positive: this workspace also resolves an unrelated,
          // transitive lru-cache@5.1.1 (via @babel/helper-compilation-targets),
          // which confuses the rule's usage detection for our direct dependency
          ignoredDependencies: ['lru-cache'],
        },
      ],
    },
    languageOptions: {
      parser: await import('jsonc-eslint-parser'),
    },
  },
]);
