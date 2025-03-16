import * as ADCSDK from '@api7/adc-sdk';
import axios, { Axios, AxiosError, AxiosResponse } from 'axios';
import {
  Observable,
  ObservableInput,
  Subject,
  catchError,
  concatMap,
  filter,
  from,
  map,
  mergeMap,
  of,
  reduce,
  switchMap,
  tap,
  throwError,
  toArray,
} from 'rxjs';
import { SemVer } from 'semver';

import { baseclass } from './fetcher';
import { FromADC } from './transformer';
import { capitalizeFirstLetter } from './utils';

export interface OperatorOptions {
  client: Axios;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  gatewayGroupName?: string;
  gatewayGroupId?: string;
}
export class Operator extends baseclass {
  private readonly client: Axios;

  constructor(private readonly opts: OperatorOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  public operate(event: ADCSDK.Event) {
    const { type, resourceType, resourceId, parentId } = event;
    const isUpdate = type !== ADCSDK.EventType.DELETE;
    const path =
      resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
        ? `consumers/${parentId}/credentials/${resourceId}`
        : `${resourceType === ADCSDK.ResourceType.STREAM_ROUTE ? 'stream_routes' : this.generateResourceTypeInAPI(resourceType)}/${resourceId}`;

    return from(
      this.client.request({
        method: 'DELETE',
        url: path,
        params: { gateway_group_id: this.opts.gatewayGroupId },
        validateStatus: () => true,
        ...(isUpdate && {
          method: 'PUT',
          data: this.fromADC(event),
        }),
      }),
    ).pipe(
      mergeMap((resp) =>
        resp?.data?.error_msg
          ? throwError(() => new Error(resp.data.error_msg))
          : of(resp),
      ),
    );
  }

  public sync(events: Array<ADCSDK.Event>) {
    return this.syncPreprocessEvents(events).pipe(
      concatMap((group) =>
        from(group).pipe(
          mergeMap((event) => {
            const taskName = this.generateTaskName(event);
            const logger = this.getLogger(taskName);
            const taskStateEvent = this.taskStateEvent(taskName);
            logger(taskStateEvent('TASK_START'));
            return from(this.operate(event)).pipe(
              tap((resp) => logger(this.debugLogEvent(resp))),
              map<AxiosResponse, ADCSDK.BackendSyncResult>((response) => {
                return {
                  success: true,
                  event,
                  axiosResponse: response,
                } satisfies ADCSDK.BackendSyncResult;
              }),
              catchError<
                ADCSDK.BackendSyncResult,
                ObservableInput<ADCSDK.BackendSyncResult>
              >((error: Error | AxiosError) => {
                //TODO error handler
                if (axios.isAxiosError(error)) {
                  if (error.response) {
                    //TODO 有响应的状态码失败
                  } else {
                    //TODO 无正常响应
                  }
                  //if (opts.exitOnFailed) throw error;
                  return of({
                    success: false,
                    event,
                    axiosResponse: error.response,
                    error,
                  } satisfies ADCSDK.BackendSyncResult);
                }
              }),
              tap(() => logger(taskStateEvent('TASK_DONE'))),
            );
          }),
        ),
      ),
    );
  }

  // Preprocess events for sync:
  // 1. Events that attempt to remove routes but not for the purpose of
  //    updating the service will be ignored.
  // 2. The service will at least be removed from the gateway group, i.e.,
  //    it will stop processing such traffic.
  // 3. Divide events into groups by resource type and operation type.
  private syncPreprocessEvents(events: Array<ADCSDK.Event>) {
    const isRouteLike = (event: ADCSDK.Event) =>
      [ADCSDK.ResourceType.ROUTE, ADCSDK.ResourceType.STREAM_ROUTE].includes(
        event.resourceType,
      );

    const event$ = from(events);
    return event$.pipe(
      // Aggregate services that need to be deleted
      filter(
        (event) =>
          event.resourceType === ADCSDK.ResourceType.SERVICE &&
          event.type === ADCSDK.EventType.DELETE,
      ),
      map((event) => event.resourceId),
      toArray(),
      // Switch to a new event pipe for event filtering and grouping.
      // It will use the deleted service ID that has been aggregated.
      switchMap((deletedServiceIds) =>
        event$.pipe(
          // If an event wants to delete a route, but its parent service
          // will also be deleted, this operation can be ignored.
          // The deletion service will cascade the deletion of the route.
          filter(
            (event) =>
              !(
                isRouteLike(event) &&
                event.type === ADCSDK.EventType.DELETE &&
                deletedServiceIds.includes(event.parentId)
              ),
          ),
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
        ),
      ),
    );
  }

  private generateTaskName = (event: ADCSDK.Event) =>
    `${capitalizeFirstLetter(
      event.type,
    )} ${event.resourceType}: "${event.resourceName}"`;

  private generateResourceTypeInAPI(resourceType: ADCSDK.ResourceType) {
    return resourceType !== ADCSDK.ResourceType.PLUGIN_METADATA
      ? `${resourceType}s`
      : ADCSDK.ResourceType.PLUGIN_METADATA;
  }

  private fromADC(event: ADCSDK.Event) {
    const fromADC = new FromADC();
    switch (event.resourceType) {
      case ADCSDK.ResourceType.CONSUMER:
        return fromADC.transformConsumer(event.newValue as ADCSDK.Consumer);
      case ADCSDK.ResourceType.GLOBAL_RULE:
        return {
          plugins: {
            [event.resourceId]: event.newValue,
          },
        };
      case ADCSDK.ResourceType.PLUGIN_METADATA:
        return event.newValue;
      case ADCSDK.ResourceType.SERVICE:
        (event.newValue as ADCSDK.Service).id = event.resourceId;
        return fromADC.transformService(event.newValue as ADCSDK.Service);
      case ADCSDK.ResourceType.ROUTE:
        (event.newValue as ADCSDK.Route).id = event.resourceId;
        return fromADC.transformRoute(
          event.newValue as ADCSDK.Route,
          event.parentId,
        );
      case ADCSDK.ResourceType.STREAM_ROUTE:
        (event.newValue as ADCSDK.StreamRoute).id = event.resourceId;
        return fromADC.transformStreamRoute(
          event.newValue as ADCSDK.StreamRoute,
          event.parentId,
        );
      case ADCSDK.ResourceType.SSL:
        (event.newValue as ADCSDK.SSL).id = event.resourceId;
        return fromADC.transformSSL(event.newValue as ADCSDK.SSL);
      case ADCSDK.ResourceType.CONSUMER_CREDENTIAL:
        (event.newValue as ADCSDK.ConsumerCredential).id = event.resourceId;
        return fromADC.transformConsumerCredential(
          event.newValue as ADCSDK.ConsumerCredential,
        );
    }
  }
}
