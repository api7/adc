import * as ADCSDK from '@api7/adc-sdk';
import axios, { Axios, AxiosError, AxiosResponse } from 'axios';
import {
  ObservableInput,
  Subject,
  catchError,
  concatMap,
  from,
  map,
  mergeMap,
  of,
  tap,
} from 'rxjs';
import { SemVer, gte as semVerGTE, lt as semVerLT } from 'semver';

import { FromADC } from './transformer';
import { capitalizeFirstLetter, resourceTypeToAPIName } from './utils';

export interface OperatorOptions {
  client: Axios;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
}
export class Operator extends ADCSDK.backend.BackendEventSource {
  private readonly client: Axios;

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
        : `${resourceType === ADCSDK.ResourceType.STREAM_ROUTE ? 'stream_routes' : resourceTypeToAPIName(resourceType)}/${resourceId}`
    }`;

    return from(
      this.client.request({
        method: 'DELETE',
        url: path,
        validateStatus: () => true,
        ...(isUpdate && {
          method: 'PUT',
          data: this.fromADC(event, this.opts.version),
        }),
      }),
    );
  }

  public sync(events: Array<ADCSDK.Event>) {
    //TODO preprocess
    return from([events]).pipe(
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
              map<AxiosResponse, ADCSDK.BackendSyncResult>((response) => {
                if (response.status >= 400)
                  throw new Error(response.data.error_msg);

                return {
                  success: true,
                  event,
                  axiosResponse: response,
                  ...(response?.data?.error_msg && {
                    success: false,
                    error: new Error(response.data.error_msg),
                  }),
                } satisfies ADCSDK.BackendSyncResult;
              }),
              catchError<
                ADCSDK.BackendSyncResult,
                ObservableInput<ADCSDK.BackendSyncResult>
              >((error: Error | AxiosError) => {
                if (axios.isAxiosError(error)) {
                  //TODO exitOnFailed if (opts.exitOnFailed) throw error;
                  return of({
                    success: false,
                    event,
                    axiosResponse: error.response,
                    error,
                  } satisfies ADCSDK.BackendSyncResult);
                }
                return of({
                  success: false,
                  event,
                  error,
                } satisfies ADCSDK.BackendSyncResult);
              }),
              tap(() => logger(taskStateEvent('TASK_DONE'))),
            );
          }),
        ),
      ),
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
    }
  }
}
