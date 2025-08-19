import * as ADCSDK from '@api7/adc-sdk';
import { type AxiosInstance } from 'axios';
import {
  type Subject,
  finalize,
  from,
  map,
  max,
  mergeMap,
  of,
  switchMap,
  tap,
} from 'rxjs';
import { type SemVer, gt as semverGT } from 'semver';

import {
  ENDPOINT_CONFIG,
  HEADER_CREDENTIAL,
  HEADER_LAST_MODIFIED,
} from './constants';
import { toADC } from './transformer';
import type * as typing from './typing';

export interface FetcherOptions {
  client: AxiosInstance;
  serverTokenMap: typing.ServerTokenMap;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  backendOpts: ADCSDK.BackendOptions;
}
export class Fetcher extends ADCSDK.backend.BackendEventSource {
  constructor(private readonly opts: FetcherOptions) {
    super();
    this.subject = opts.eventSubject;
  }

  public dump() {
    type result = [ADCSDK.Configuration, typing.APISIXStandalone];

    const taskName = `Fetch configuration`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(this.findLatest()).pipe(
      switchMap((server) => {
        if (!server) return of([{}, {}] as result);
        return from(
          this.opts.client.get<typing.APISIXStandalone>(
            `${server}${ENDPOINT_CONFIG}`,
            {
              headers: {
                [HEADER_CREDENTIAL]: this.opts.serverTokenMap.get(server),
              },
            },
          ),
        ).pipe(
          tap((resp) => logger(this.debugLogEvent(resp))),
          map((resp) => [toADC(resp.data), resp.data] as result),
        );
      }),
      finalize(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  // Gets the latest configuration from the `X-Last-Modified` in the response header,
  // which values the timestamp of when the last update was accepted.
  private findLatest() {
    const taskName = `Find server with the latest config`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(this.opts.serverTokenMap).pipe(
      mergeMap(([server, token]) => {
        return from(
          (semverGT(this.opts.version, '3.13.0')
            ? this.opts.client.head
            : this.opts.client.get)(`${server}${ENDPOINT_CONFIG}`, {
            headers: { [HEADER_CREDENTIAL]: token },
          }),
        ).pipe(
          tap((resp) => logger(this.debugLogEvent(resp))),
          map((res) => ({
            server,
            timestamp: parseInt(res.headers[HEADER_LAST_MODIFIED] ?? 0),
          })),
        );
      }),
      max((a, b) => (a.timestamp < b.timestamp ? -1 : 1)),
      map(({ server, timestamp }) => {
        return timestamp > 0 ? server : undefined;
      }),
      finalize(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }
}
