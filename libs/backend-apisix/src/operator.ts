import * as ADCSDK from '@api7/adc-sdk';
import axios, { AxiosError, type AxiosInstance, AxiosResponse } from 'axios';
import {
  Observable,
  ObservableInput,
  Subject,
  catchError,
  concatMap,
  delay,
  from,
  map,
  mergeMap,
  of,
  reduce,
  retry,
  tap,
  throwError,
} from 'rxjs';
import { SemVer, gte as semVerGTE, lt as semVerLT } from 'semver';

import { FromADC } from './transformer';
import * as typing from './typing';
import { capitalizeFirstLetter, resourceTypeToAPIName } from './utils';

export interface OperatorOptions {
  client: AxiosInstance;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
}
export class Operator extends ADCSDK.backend.BackendEventSource {
  private readonly client: AxiosInstance;

  constructor(private readonly opts: OperatorOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  private operate(event: ADCSDK.Event) {
    const { type, resourceType, resourceId, parentId } = event;
    const isUpdate = type !== ADCSDK.EventType.DELETE;
    const path = `/apisix/admin/${
      resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
        ? `consumers/${parentId}/credentials/${resourceId}`
        : `${resourceTypeToAPIName(resourceType)}/${resourceId}`
    }`;

    // Handle service operations separately
    if (resourceType === ADCSDK.ResourceType.SERVICE) {
      return this.operateService(event, path);
    }

    return from(
      this.client.request({
        method: 'DELETE',
        url: path,
        ...(isUpdate && {
          method: 'PUT',
          data: this.fromADC(event, this.opts.version),
        }),
      }),
    );
  }

  private operateService(event: ADCSDK.Event, servicePath: string) {
    const { type } = event;

    // Handle service deletion
    if (type === ADCSDK.EventType.DELETE) {
      // Delete service with upstream: delete service first, then upstream
      if (event.oldValue && (event.oldValue as ADCSDK.Service).upstream) {
        return this.deleteServiceWithUpstream(event, servicePath);
      }
      return from(this.client.request({ url: servicePath, method: 'DELETE' }));
    }

    // Handle service create/update
    const data = this.fromADC(event, this.opts.version) as typing.Service;
    const oldUpstream = event.oldValue
      ? (event.oldValue as ADCSDK.Service).upstream
      : undefined;
    const newUpstream = (event.newValue as ADCSDK.Service).upstream;

    // oldValue has upstream, newValue doesn't -> delete upstream
    if (oldUpstream && !newUpstream) {
      return this.deleteUpstreamThenUpdateService(event, data, servicePath);
    }

    // newValue has upstream -> create/update upstream
    if (newUpstream) {
      return this.upsertServiceWithUpstream(event, data, servicePath);
    }

    // Service without upstream
    return from(this.client.request({ url: servicePath, method: 'PUT', data }));
  }

  private getUpstreamUrl(upstreamId: string) {
    return `/apisix/admin/upstreams/${upstreamId}`;
  }

  private upsertServiceWithUpstream(
    event: ADCSDK.Event,
    data: typing.Service,
    servicePath: string,
  ) {
    // Create/Update upstream first, then service
    return from(
      this.client.request({
        url: this.getUpstreamUrl(event.resourceId),
        method: 'PUT',
        data: {
          ...data.upstream,
          id: event.resourceId,
          name: event.resourceName,
        },
      }),
    ).pipe(
      concatMap(() =>
        this.client.request({
          url: servicePath,
          method: 'PUT',
          data: { ...data, upstream: undefined, upstream_id: event.resourceId },
        }),
      ),
    );
  }

  private deleteUpstreamWithRetry(upstreamId: string) {
    return from(
      this.client.request({
        url: this.getUpstreamUrl(upstreamId),
        method: 'DELETE',
      }),
    ).pipe(
      retry({
        count: 3,
        delay: (error: Error | AxiosError, retryCount: number) => {
          if (
            axios.isAxiosError(error) &&
            error.response?.data?.error_msg?.includes('is still using it')
          ) {
            return of(null).pipe(delay(100 * Math.pow(2, retryCount - 1)));
          }
          return throwError(() => error);
        },
      }),
    );
  }

  private deleteServiceWithUpstream(event: ADCSDK.Event, servicePath: string) {
    return this.executeServiceOpThenDeleteUpstream(
      { url: servicePath, method: 'DELETE' },
      event.resourceId,
    );
  }

  private deleteUpstreamThenUpdateService(
    event: ADCSDK.Event,
    data: typing.Service,
    servicePath: string,
  ) {
    return this.executeServiceOpThenDeleteUpstream(
      { url: servicePath, method: 'PUT', data },
      event.resourceId,
    );
  }

  private executeServiceOpThenDeleteUpstream(
    serviceRequest: {
      url: string;
      method: 'DELETE' | 'PUT';
      data?: typing.Service;
    },
    upstreamId: string,
  ) {
    return from(this.client.request(serviceRequest)).pipe(
      concatMap(() => this.deleteUpstreamWithRetry(upstreamId)),
    );
  }

  public sync(
    events: Array<ADCSDK.Event>,
    opts: ADCSDK.BackendSyncOptions = { exitOnFailure: true },
  ) {
    return this.syncPreprocessEvents(events).pipe(
      concatMap((group) =>
        from(group).pipe(
          mergeMap((event) => {
            // Compatibility check
            if (
              semVerLT(this.opts.version, '3.7.0') &&
              event.resourceType === ADCSDK.ResourceType.STREAM_ROUTE
            )
              return of({
                success: false,
                event,
                error: new Error(
                  'The stream routes on versions below 3.7.0 are not supported as they are not supported configured on the service.',
                ),
              } as ADCSDK.BackendSyncResult);

            if (
              semVerLT(this.opts.version, '3.11.0') &&
              event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
            )
              return of({
                success: false,
                event,
                error: new Error(
                  'The consumer credentials are only supported in Apache APISIX version 3.11 and above.',
                ),
              } as ADCSDK.BackendSyncResult);

            const taskName = this.generateTaskName(event);
            const logger = this.getLogger(taskName);
            const taskStateEvent = this.taskStateEvent(taskName);
            logger(taskStateEvent('TASK_START'));
            return from(this.operate(event)).pipe(
              tap((resp) => logger(this.debugLogEvent(resp))),
              map<AxiosResponse, ADCSDK.BackendSyncResult>(
                (response) =>
                  ({
                    success: true,
                    event,
                    axiosResponse: response,
                  }) satisfies ADCSDK.BackendSyncResult,
              ),
              catchError<
                ADCSDK.BackendSyncResult,
                ObservableInput<ADCSDK.BackendSyncResult>
              >((error: Error | AxiosError) => {
                if (opts.exitOnFailure) {
                  if (axios.isAxiosError(error) && error.response)
                    return throwError(
                      () =>
                        new Error(
                          error.response?.data?.error_msg ??
                            JSON.stringify(error.response?.data),
                        ),
                    );
                  return throwError(() => error);
                }
                return of({
                  success: false,
                  event,
                  error,
                  ...(axios.isAxiosError(error) && {
                    axiosResponse: error.response,
                    ...(error.response?.data?.error_msg && {
                      error: new Error(error.response.data.error_msg),
                    }),
                  }),
                } satisfies ADCSDK.BackendSyncResult);
              }),
              tap(() => logger(taskStateEvent('TASK_DONE'))),
            );
          }, opts.concurrent),
        ),
      ),
    );
  }

  // Preprocess events for sync:
  // 1. Divide events into groups by resource type and operation type.
  private syncPreprocessEvents(events: Array<ADCSDK.Event>) {
    return from(events).pipe(
      // Grouping events by resource type and operation type.
      // The sequence of events should not be broken in this process,
      // and the correct behavior of the API will depend on the order
      // of execution.
      reduce((groups, event) => {
        const key = `${event.resourceType}.${event.type}`;
        (groups[key] = groups[key] || []).push(event);
        return groups;
      }, {}),
      // Strip group name and convert to two-dims arrays
      // {"service.create": [1], "consumer.create": [2]} => [[1], [2]]
      mergeMap<
        Record<string, Array<ADCSDK.Event>>,
        Observable<Array<ADCSDK.Event>>
      >((obj) => from(Object.values(obj))),
    );
  }

  private generateTaskName(event: ADCSDK.Event) {
    return `${capitalizeFirstLetter(
      event.type,
    )} ${event.resourceType}: "${event.resourceName}"`;
  }

  private fromADC(event: ADCSDK.Event, version: SemVer) {
    const fromADC = new FromADC();
    switch (event.resourceType) {
      case ADCSDK.ResourceType.CONSUMER:
        return fromADC.transformConsumer(event.newValue as ADCSDK.Consumer);
      case ADCSDK.ResourceType.CONSUMER_GROUP:
        (event.newValue as ADCSDK.ConsumerGroup).id = event.resourceId;
        return fromADC.transformConsumerGroup(
          event.newValue as ADCSDK.ConsumerGroup,
        )[0];
      case ADCSDK.ResourceType.CONSUMER_CREDENTIAL:
        (event.newValue as ADCSDK.ConsumerCredential).id = event.resourceId;
        return fromADC.transformConsumerCredential(
          event.newValue as ADCSDK.ConsumerCredential,
        );
      case ADCSDK.ResourceType.GLOBAL_RULE:
        return {
          plugins: {
            [event.resourceId]: event.newValue,
          },
        };
      case ADCSDK.ResourceType.PLUGIN_METADATA:
        return event.newValue;
      case ADCSDK.ResourceType.ROUTE: {
        (event.newValue as ADCSDK.Route).id = event.resourceId;
        const route = fromADC.transformRoute(
          event.newValue as ADCSDK.Route,
          event.parentId,
        );
        if (event.parentId) route.service_id = event.parentId;
        return route;
      }
      case ADCSDK.ResourceType.SERVICE:
        (event.newValue as ADCSDK.Service).id = event.resourceId;
        return fromADC.transformService(event.newValue as ADCSDK.Service);
      case ADCSDK.ResourceType.SSL:
        (event.newValue as ADCSDK.SSL).id = event.resourceId;
        return fromADC.transformSSL(event.newValue as ADCSDK.SSL);
      case ADCSDK.ResourceType.STREAM_ROUTE: {
        (event.newValue as ADCSDK.StreamRoute).id = event.resourceId;
        const route = fromADC.transformStreamRoute(
          event.newValue as ADCSDK.StreamRoute,
          event.parentId,
          semVerGTE(version, '3.8.0'),
        );
        if (event.parentId) route.service_id = event.parentId;
        return route;
      }
      case ADCSDK.ResourceType.UPSTREAM: {
        const upstream = fromADC.transformUpstream(
          event.newValue as ADCSDK.Upstream,
        );
        if (event.parentId)
          upstream.labels = {
            ...upstream.labels,
            [typing.ADC_UPSTREAM_SERVICE_ID_LABEL]: event.parentId,
          };
        return upstream;
      }
    }
  }
}
