/**
 * Export jsonschema file by:
 *
 * $ nx export-schema cli
 *
 */
import { ConfigurationSchema } from '@api7/adc-sdk/schema';
import { writeFileSync } from 'fs';
import { z } from 'zod';

writeFileSync(
  'schema.json',
  JSON.stringify(z.toJSONSchema(ConfigurationSchema), null, 2) + '\n',
);
