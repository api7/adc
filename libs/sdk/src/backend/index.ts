import { AxiosResponse } from 'axios';
import EventEmitter from 'events';
import { Listr } from 'listr2';
import { Observable } from 'rxjs';

import * as ADCSDK from '..';

export interface BackendOptions {
  logger: ADCSDK.Logger;

  server: string;
  token: string;
  gatewayGroup?: string;
  timeout?: number;
  caCertFile?: string;
  tlsClientCertFile?: string;
  tlsClientKeyFile?: string;
  tlsSkipVerify?: boolean;
  verbose?: number;

  labelSelector?: Record<string, string>;
  includeResourceType?: Array<ADCSDK.ResourceType>;
  excludeResourceType?: Array<ADCSDK.ResourceType>;
}

export const BackendEventType = {
  TASK_START: 'TASK_START',
  TASK_DONE: 'TASK_DONE',
  AXIOS_DEBUG: 'AXIOS_DEBUG',
} as const;

export interface BackendEventTaskState {
  name?: string;
}

export interface BackendEventAxiosDebug {
  response: AxiosResponse;
  description?: string;
}

export interface BackendSyncResult {
  success: boolean;
  event: ADCSDK.Event;
  axiosResponse?: AxiosResponse;
  error?: Error;
}

export interface Backend {
  ping: () => Promise<void>;

  defaultValue: () => Promise<ADCSDK.DefaultValue>;
  dump0: () => Promise<Listr<{ remote: ADCSDK.Configuration }>>;
  dump: () => Observable<ADCSDK.Configuration>;
  sync: (events: Array<ADCSDK.Event>) => Observable<BackendSyncResult>;

  supportValidate?: () => Promise<boolean>;
  supportStreamRoute?: () => Promise<boolean>;

  // Event report: optional standard, backends may not implement event reporting.
  // They will therefore lose output related to task start, end and verbose logs.
  on(
    eventType: typeof ADCSDK.BackendEventType.AXIOS_DEBUG,
    cb: (args: ADCSDK.BackendEventAxiosDebug) => void,
  ): void;
  on(
    eventType: typeof ADCSDK.BackendEventType.TASK_START,
    cb: (args: ADCSDK.BackendEventTaskState) => void,
  ): void;
  on(
    eventType: typeof ADCSDK.BackendEventType.TASK_DONE,
    cb: (args: ADCSDK.BackendEventTaskState) => void,
  ): void;
  removeAllListeners?: () => void;
}
