import { AxiosResponse } from 'axios';
import { Observable, Subscription } from 'rxjs';
import { SemVer } from 'semver';

import * as ADCSDK from '..';
import { BackendEventSource } from './utils';

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

  labelSelector?: Record<string, string>;
  includeResourceType?: Array<ADCSDK.ResourceType>;
  excludeResourceType?: Array<ADCSDK.ResourceType>;

  cacheKey?: string;
}

export const BackendEventType = {
  TASK_START: 'TASK_START',
  TASK_DONE: 'TASK_DONE',
  AXIOS_DEBUG: 'AXIOS_DEBUG',
} as const;

export type BackendEvent =
  | {
      type: typeof BackendEventType.AXIOS_DEBUG;
      event: BackendEventAxiosDebug;
    }
  | {
      type:
        | typeof BackendEventType.TASK_START
        | typeof BackendEventType.TASK_DONE;
      event: BackendEventTaskState;
    };

export interface BackendEventTaskState {
  name?: string;
}

export interface BackendEventAxiosDebug {
  response: AxiosResponse;
  description?: string;
}

export interface BackendSyncOptions {
  concurrent?: number;
  exitOnFailure?: boolean;
}

export interface BackendSyncResult {
  success: boolean;
  event: ADCSDK.Event;
  axiosResponse?: AxiosResponse;
  error?: Error;
  server?: string;
}

export interface BackendMetadata {
  logScope: string[];
}

export interface Backend {
  metadata: () => BackendMetadata;

  ping: () => Promise<void>;

  version: () => Promise<SemVer>;
  defaultValue: () => Promise<ADCSDK.DefaultValue>;
  dump: () => Observable<ADCSDK.Configuration>;
  sync: (
    events: Array<ADCSDK.Event>,
    opts?: BackendSyncOptions,
  ) => Observable<BackendSyncResult>;

  supportValidate?: () => Promise<boolean>;
  supportStreamRoute?: () => Promise<boolean>;

  // Event report: optional standard, backends may not implement event reporting.
  // They will therefore lose output related to task start, end and verbose logs.
  on(
    eventType: typeof ADCSDK.BackendEventType.AXIOS_DEBUG,
    cb: (args: ADCSDK.BackendEventAxiosDebug) => void,
  ): Subscription;
  on(
    eventType: typeof ADCSDK.BackendEventType.TASK_START,
    cb: (args: ADCSDK.BackendEventTaskState) => void,
  ): Subscription;
  on(
    eventType: typeof ADCSDK.BackendEventType.TASK_DONE,
    cb: (args: ADCSDK.BackendEventTaskState) => void,
  ): Subscription;
}

export default Backend;
export const backend = {
  BackendEventSource,
};
