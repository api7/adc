import * as ADCSDK from '@api7/adc-sdk';
import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosResponse,
} from 'axios';
import {
  Observable,
  ObservableInput,
  Subject,
  catchError,
  concatMap,
  filter,
  forkJoin,
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

import { FromADC } from './transformer';
import * as typing from './typing';
import { capitalizeFirstLetter } from './utils';

export interface OperatorOptions {
  client: AxiosInstance;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  gatewayGroupName?: string;
  gatewayGroupId?: string;
}
export class Operator extends ADCSDK.backend.BackendEventSource {
  private readonly client: AxiosInstance;

  // Memoized control-plane-global custom plugin list, used to reconcile group
  // membership during a sync (resolve ids, preserve/trim gateway_groups).
  private customPluginListCache?: Promise<Array<typing.CustomPlugin>>;

  constructor(private readonly opts: OperatorOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  private operate(event: ADCSDK.Event) {
    const { type, resourceType, resourceId, parentId } = event;

    // Custom plugins are control-plane-global resources addressed under
    // "/api/custom_plugins" (not the gateway-group-scoped admin API). They are
    // reconciled by membership rather than created/deleted outright.
    if (resourceType === ADCSDK.ResourceType.CUSTOM_PLUGIN)
      return from(this.operateCustomPlugin(event));

    const isUpdate = type !== ADCSDK.EventType.DELETE;
    const path = `/apisix/admin/${
      resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
        ? `consumers/${parentId}/credentials/${resourceId}`
        : resourceType === ADCSDK.ResourceType.UPSTREAM
          ? `services/${parentId}/upstreams/${resourceId}`
          : `${resourceType === ADCSDK.ResourceType.STREAM_ROUTE ? 'stream_routes' : this.generateResourceTypeInAPI(resourceType)}/${resourceId}`
    }`;

    return from(
      this.client.request({
        method: 'DELETE',
        url: path,
        params: { gateway_group_id: this.opts.gatewayGroupId },
        ...(isUpdate && {
          method: 'PUT',
          data: this.fromADC(event),
        }),
      }),
    );
  }

  public sync(events: Array<ADCSDK.Event>, opts: ADCSDK.BackendSyncOptions) {
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
                        new Error(ADCSDK.utils.formatAxiosErrorMessage(error)),
                    );
                  return throwError(() => error);
                }
                return of({
                  success: false,
                  event,
                  error,
                  ...(axios.isAxiosError(error) && {
                    axiosResponse: error.response,
                    ...(error.response && {
                      error: new Error(
                        ADCSDK.utils.formatAxiosErrorMessage(error),
                      ),
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
    return forkJoin({
      // Aggregate services that need to be deleted
      deletedServiceIds: event$.pipe(
        filter(
          (event) =>
            event.resourceType === ADCSDK.ResourceType.SERVICE &&
            event.type === ADCSDK.EventType.DELETE,
        ),
        map((event) => event.resourceId),
        toArray(),
      ),
      // Aggregate consumers that need to be deleted
      deletedConsumerIds: event$.pipe(
        filter(
          (event) =>
            event.resourceType === ADCSDK.ResourceType.CONSUMER &&
            event.type === ADCSDK.EventType.DELETE,
        ),
        map((event) => event.resourceId),
        toArray(),
      ),
    }).pipe(
      // Switch to a new event pipe for event filtering and grouping.
      // It will use the deleted service ID that has been aggregated.
      switchMap(({ deletedServiceIds, deletedConsumerIds }) =>
        event$.pipe(
          filter(
            (event) =>
              // If an event wants to delete a route, but its parent service
              // will also be deleted, this operation can be ignored.
              // The deletion service will cascade the deletion of the route.
              !(
                isRouteLike(event) &&
                event.type === ADCSDK.EventType.DELETE &&
                event.parentId &&
                deletedServiceIds.includes(event.parentId)
              ) &&
              // If an event wants to delete an upstream, but its parent service
              // will also be deleted, this operation can be ignored.
              // The deletion service will cascade the deletion of the upstream.
              // This does not affect inline upstream processing, which is always
              // handled on the server side.
              !(
                event.resourceType === ADCSDK.ResourceType.UPSTREAM &&
                event.type === ADCSDK.EventType.DELETE &&
                event.parentId &&
                deletedServiceIds.includes(event.parentId)
              ),
          ),
          // If an event wants to delete a consumer credential, but its parent
          // consumer will also be deleted, this operation can be ignored.
          // The deletion consumer will cascade the deletion of the credential.
          filter(
            (event) =>
              !(
                event.resourceType ===
                  ADCSDK.ResourceType.CONSUMER_CREDENTIAL &&
                event.type === ADCSDK.EventType.DELETE &&
                event.parentId &&
                deletedConsumerIds.includes(event.parentId)
              ),
          ),
          // Grouping events by resource type and operation type.
          // The sequence of events should not be broken in this process,
          // and the correct behavior of the API will depend on the order
          // of execution.
          reduce(
            (groups, event) => {
              const key = `${event.resourceType}.${event.type}` as const;
              (groups[key] ??= []).push(event);
              return groups;
            },
            {} as Record<
              `${ADCSDK.ResourceType}.${ADCSDK.EventType}`,
              Array<ADCSDK.Event>
            >,
          ),
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
          event.parentId!,
        );
      case ADCSDK.ResourceType.STREAM_ROUTE:
        (event.newValue as ADCSDK.StreamRoute).id = event.resourceId;
        return fromADC.transformStreamRoute(
          event.newValue as ADCSDK.StreamRoute,
          event.parentId!,
        );
      case ADCSDK.ResourceType.SSL:
        (event.newValue as ADCSDK.SSL).id = event.resourceId;
        return fromADC.transformSSL(event.newValue as ADCSDK.SSL);
      case ADCSDK.ResourceType.CONSUMER_CREDENTIAL:
        (event.newValue as ADCSDK.ConsumerCredential).id = event.resourceId;
        return fromADC.transformConsumerCredential(
          event.newValue as ADCSDK.ConsumerCredential,
        );
      case ADCSDK.ResourceType.UPSTREAM:
        return fromADC.transformUpstream(event.newValue as ADCSDK.Upstream);
      default:
        throw new Error(`Unsupported resource type: ${event.resourceType}`);
    }
  }

  private getCustomPluginList(): Promise<Array<typing.CustomPlugin>> {
    if (!this.customPluginListCache)
      this.customPluginListCache = this.client
        .get<typing.ListResponse<typing.CustomPlugin>>('/api/custom_plugins')
        .then((resp) => resp.data?.list ?? []);
    return this.customPluginListCache;
  }

  // Reconciles a custom plugin against the control plane while only touching
  // the gateway group this backend targets:
  // - create/update: ensure this group is in the plugin's membership (POST when
  //   the plugin does not exist yet, otherwise PUT and append the group).
  // - delete (prune): drop this group from the membership; the plugin is only
  //   removed entirely when no other group still references it.
  // Because a custom plugin is shared across its member groups, the uploaded
  // source/metadata in the local config is authoritative for every group.
  private async operateCustomPlugin(
    event: ADCSDK.Event,
  ): Promise<AxiosResponse> {
    const groupId = this.opts.gatewayGroupId;
    if (!groupId)
      throw new Error(
        'Managing custom plugins requires a resolved gateway group, but none is available for the current backend.',
      );

    const name = event.resourceName;
    const existing = (await this.getCustomPluginList()).find(
      (plugin) => plugin.name === name,
    );
    const fromADC = new FromADC();

    if (event.type === ADCSDK.EventType.DELETE) {
      if (!existing)
        return this.syntheticResponse('delete', 'custom plugin already absent');

      const remaining = (existing.gateway_groups ?? []).filter(
        (id) => id !== groupId,
      );
      if (remaining.length === 0)
        return this.client.request({
          method: 'DELETE',
          url: `/api/custom_plugins/${existing.id}`,
        });

      return this.client.request({
        method: 'PUT',
        url: `/api/custom_plugins/${existing.id}`,
        data: {
          ...fromADC.transformCustomPlugin(
            event.oldValue as ADCSDK.CustomPlugin,
          ),
          gateway_groups: remaining,
        },
      });
    }

    const body = fromADC.transformCustomPlugin(
      event.newValue as ADCSDK.CustomPlugin,
    );
    if (existing)
      return this.client.request({
        method: 'PUT',
        url: `/api/custom_plugins/${existing.id}`,
        data: {
          ...body,
          gateway_groups: Array.from(
            new Set([...(existing.gateway_groups ?? []), groupId]),
          ),
        },
      });

    return this.client.request({
      method: 'POST',
      url: '/api/custom_plugins',
      data: { ...body, gateway_groups: [groupId] },
    });
  }

  // A no-op result for the case where a pruned plugin is already absent, shaped
  // so the shared debug/logging path can render it without a real request.
  private syntheticResponse(method: string, message: string): AxiosResponse {
    return {
      status: 200,
      statusText: 'OK',
      headers: {},
      config: { method, url: '/api/custom_plugins', headers: {} },
      data: { value: { message } },
    } as unknown as AxiosResponse;
  }
}
