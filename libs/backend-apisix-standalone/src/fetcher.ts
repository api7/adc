import * as ADCSDK from '@api7/adc-sdk';
import { type AxiosInstance } from 'axios';
import { type Subject, finalize, from, map, tap } from 'rxjs';
import { type SemVer } from 'semver';

import { toADC } from './transformer';
import type * as typing from './typing';

export interface FetcherOptions {
  client: AxiosInstance;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  backendOpts: ADCSDK.BackendOptions;
}
export class Fetcher extends ADCSDK.backend.BackendEventSource {
  private readonly client: AxiosInstance;
  public _dump?: typing.APISIXStandaloneWithConfVersionType;

  constructor(opts: FetcherOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  public dump() {
    const taskName = `Fetch configuration`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<typing.APISIXStandaloneType>('/apisix/admin/configs'),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      tap((resp) => {
        this._dump = resp.data;
      }),
      map((resp) => toADC(resp.data)),
      finalize(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }
}
