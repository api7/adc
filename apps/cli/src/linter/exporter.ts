/**
 * Export jsonschema file by:
 *
 * $ ts-node apps/cli/src/linter/exporter.ts
 *
 */
import { writeFileSync } from 'fs';
import zodToJsonSchema from 'zod-to-json-schema';

import { ConfigurationSchema } from './schema';

writeFileSync(
  'schema.json',
  JSON.stringify(zodToJsonSchema(ConfigurationSchema), null, 2) + '\n',
);
