import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { z } from 'zod';

import { ConfigurationSchema } from '../schema';

describe('Schema JSON Check', () => {
  it('should check schema.json is consistent with git HEAD', () => {
    const currentSchema = z.toJSONSchema(ConfigurationSchema);
    const file = readFileSync(join('./schema.json'));
    expect(JSON.parse(file.toString())).toEqual(currentSchema);
  });
});
