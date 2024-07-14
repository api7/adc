import { Configuration } from '@api7/adc-sdk';

import { ConfigurationSchema } from './schema';

export const check = (config: Configuration) => {
  return ConfigurationSchema.safeParse(config);
};
