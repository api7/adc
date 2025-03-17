import * as ADCSDK from '@api7/adc-sdk';
import { Axios, AxiosResponse } from 'axios';
import { produce } from 'immer';
import { curry, isEmpty } from 'lodash';
import {
  Subject,
  combineLatest,
  from,
  map,
  mergeMap,
  of,
  takeLast,
  tap,
  toArray,
} from 'rxjs';
import { SemVer } from 'semver';

import { ToADC } from './transformer';
import * as typing from './typing';

export class baseclass {
  protected subject: Subject<ADCSDK.BackendEvent>;
  protected _innerSubject: Subject<{
    name: string;
    event: ADCSDK.BackendEvent;
  }> = new Subject();
  protected _taskEvents: Map<string, Array<ADCSDK.BackendEvent>> = new Map();

  constructor() {
    this._innerSubject.subscribe({
      next: ({ name, event }) => {
        if (!this._taskEvents.has(name)) this._taskEvents.set(name, []);
        this._taskEvents.get(name).push(event);

        if (event.type === 'TASK_DONE')
          this._taskEvents
            .get(name)
            .forEach((event) => this.subject.next(event));
      },
    });
  }

  protected getLogger = curry((name: string, event: ADCSDK.BackendEvent) => {
    this._innerSubject.next({ name, event });
  });

  protected taskStateEvent = curry(
    (
      name: string,
      type:
        | typeof ADCSDK.BackendEventType.TASK_START
        | typeof ADCSDK.BackendEventType.TASK_DONE,
    ): ADCSDK.BackendEvent => ({
      type,
      event: { name },
    }),
  );

  protected debugLogEvent = (
    response: AxiosResponse,
    description?: string,
  ): ADCSDK.BackendEvent => ({
    type: ADCSDK.BackendEventType.AXIOS_DEBUG,
    event: { response, description },
  });
}

export interface FetcherOptions {
  client: Axios;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  backendOpts: ADCSDK.BackendOptions;
  gatewayGroupName?: string;
  gatewayGroupId?: string;
}
export class Fetcher extends baseclass {
  private readonly toADC = new ToADC();
  private readonly client: Axios;

  constructor(private readonly opts: FetcherOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  public listServices() {
    if (this.isSkip(ADCSDK.ResourceType.SERVICE))
      return of<Array<ADCSDK.Service>>([]);

    const taskName = 'Fetch services';
    const taskStateEvent = this.taskStateEvent(taskName);
    const logger = this.getLogger(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<typing.ListResponse<typing.Service>>(
        `/apisix/admin/services`,
        {
          params: this.attachLabelSelector({
            gateway_group_id: this.opts.gatewayGroupId,
          }),
        },
      ),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      mergeMap((resp) =>
        from(resp.data.list).pipe(
          mergeMap((service) => {
            const params = {
              gateway_group_id: this.opts.gatewayGroupId,
              service_id: service.id,
            };
            return from(
              service.type === 'stream'
                ? this.client.get<typing.ListResponse<typing.StreamRoute>>(
                    `/apisix/admin/stream_routes`,
                    { params },
                  )
                : this.client.get<typing.ListResponse<typing.Route>>(
                    `/apisix/admin/routes`,
                    { params },
                  ),
            ).pipe(
              tap((resp) =>
                logger(
                  this.debugLogEvent(
                    resp,
                    `Get ${service.type === 'stream' ? 'stream routes' : 'routes'} in service "${service.name}"`,
                  ),
                ),
              ),
              map((resp) => {
                const routes = resp.data.list;
                return produce(service, (draft) => {
                  if (service.type === 'stream')
                    draft.stream_routes = routes as typing.StreamRoute[];
                  if (service.type === 'http')
                    service.routes = routes as typing.Route[];
                });
              }),
            );
          }),
          map((service) => this.toADC.transformService(service)),
        ),
      ),
      toArray(),
      tap(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  public listConsumers() {
    if (this.isSkip(ADCSDK.ResourceType.CONSUMER))
      return of<Array<ADCSDK.Consumer>>([]);

    const taskName = 'Fetch consumers';
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<{ list: Array<typing.Consumer> }>(
        '/apisix/admin/consumers',
        {
          params: this.attachLabelSelector({
            gateway_group_id: this.opts.gatewayGroupId,
          }),
        },
      ),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      mergeMap((resp) =>
        from(resp.data.list).pipe(
          mergeMap((consumer) =>
            from(
              this.client.get<{
                list: Array<typing.ConsumerCredential>;
              }>(`/apisix/admin/consumers/${consumer.username}/credentials`, {
                // In the current design, the consumer's credentials are not filtered
                // using labels because nested labels filters can be misleading. Even
                // if labels set for the consumer, the labels filter is not attached.
                params: { gateway_group_id: this.opts.gatewayGroupId },
              }),
            ).pipe(
              tap((resp) =>
                logger(
                  this.debugLogEvent(
                    resp,
                    `Get credentials of consumer "${consumer.username}"`,
                  ),
                ),
              ),
              map((resp) =>
                produce(consumer, (draft) => {
                  draft.credentials = resp?.data?.list;
                }),
              ),
            ),
          ),
        ),
      ),
      map((consumer) => this.toADC.transformConsumer(consumer)),
      toArray(),
      tap(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  public listSSLs() {
    if (this.isSkip(ADCSDK.ResourceType.SSL)) return of<Array<ADCSDK.SSL>>([]);

    const taskName = 'Fetch ssls';
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<{ list: Array<typing.SSL> }>('/apisix/admin/ssls', {
        params: this.attachLabelSelector({
          gateway_group_id: this.opts.gatewayGroupId,
        }),
      }),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      mergeMap((resp) =>
        from(resp.data.list).pipe(map((ssl) => this.toADC.transformSSL(ssl))),
      ),
      toArray(),
      tap(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  public listGlobalRules() {
    if (this.isSkip(ADCSDK.ResourceType.GLOBAL_RULE))
      return of<Record<string, ADCSDK.GlobalRule>>({});

    const taskName = 'Fetch global rules';
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<{ list: Array<typing.GlobalRule> }>(
        '/apisix/admin/global_rules',
        {
          params: this.attachLabelSelector({
            gateway_group_id: this.opts.gatewayGroupId,
          }),
        },
      ),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      map((resp) => this.toADC.transformGlobalRule(resp?.data?.list ?? [])),
      tap(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  public listMetadatas() {
    if (this.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA))
      return of<Record<string, ADCSDK.PluginMetadata>>({});

    const taskName = 'Fetch plugin metadata';
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<{
        value: ADCSDK.Plugins;
      }>('/apisix/admin/plugin_metadata', {
        params: this.attachLabelSelector({
          gateway_group_id: this.opts.gatewayGroupId,
        }),
      }),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      map((resp) => this.toADC.transformPluginMetadatas(resp?.data?.value)),
      tap(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  public allTask() {
    return combineLatest([
      this.listServices(),
      this.listConsumers(),
      this.listSSLs(),
      this.listGlobalRules(),
      this.listMetadatas(),
    ]).pipe(
      takeLast(1),
      map(
        ([services, consumers, ssls, global_rules, plugin_metadata]) =>
          ({
            services,
            consumers,
            ssls,
            global_rules,
            plugin_metadata,
          }) as ADCSDK.Configuration,
      ),
    );
  }

  private isSkip(type: ADCSDK.ResourceType): boolean {
    const { includeResourceType, excludeResourceType } =
      this.opts.backendOpts || {};
    if (!isEmpty(includeResourceType) && !includeResourceType.includes(type)) {
      return true;
    }
    if (!isEmpty(excludeResourceType) && excludeResourceType.includes(type)) {
      return true;
    }
    return false;
  }

  private attachLabelSelector(
    params: Record<string, string> = {},
  ): Record<string, string> {
    const { labelSelector } = this.opts.backendOpts || {};
    if (labelSelector)
      Object.entries(labelSelector).forEach(([key, value]) => {
        params[`labels[${key}]`] = value;
      });
    return params;
  }
}
