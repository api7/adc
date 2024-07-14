import { Listr } from 'listr2';

import { Configuration } from '../core';

export interface Converter {
  toADC: (input: unknown) => Listr<{
    local: Configuration;
  }>;
}
