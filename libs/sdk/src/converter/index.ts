import { Observable } from 'rxjs';

import { Configuration } from '../core';

export interface Converter {
  toADC: (input: string) => Observable<Configuration>;
}
