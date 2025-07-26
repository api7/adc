/**
 * Export jsonschema file by:
 *
 * $ nx export-schema cli
 *
 */
import { writeFileSync } from 'fs';
import { z } from 'zod';

import { ConfigurationSchema } from './schema';

writeFileSync(
  'schema.json',
  JSON.stringify(z.toJSONSchema(ConfigurationSchema), null, 2) + '\n',
);
