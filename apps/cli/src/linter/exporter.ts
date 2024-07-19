import { writeFileSync } from 'fs';
import zodToJsonSchema from 'zod-to-json-schema';

import { ConfigurationSchema } from './schema';

writeFileSync(
  'schema.json',
  JSON.stringify(zodToJsonSchema(ConfigurationSchema)),
);
