import { Listr } from 'listr2';

import * as ADCSDK from '..';

export interface BackendOptions {
  server: string;
  token: string;
  gatewayGroup?: string;
  timeout?: number;
  caCertFile?: string;
  tlsClientCertFile?: string;
  tlsClientKeyFile?: string;
  tlsSkipVerify?: boolean;
  verbose?: number;
}

export interface Backend {
  ping: () => Promise<void>;

  dump: () => Promise<Listr<{ remote: ADCSDK.Configuration }>>;
  sync: () => Promise<Listr>;

  supportValidate?: () => Promise<boolean>;
  supportStreamRoute?: () => Promise<boolean>;
}
