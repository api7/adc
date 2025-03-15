import * as ADCSDK from '@api7/adc-sdk';
import { Axios, AxiosResponse } from 'axios';
import { produce } from 'immer';
import { isEmpty } from 'lodash';
import EventEmitter from 'node:events';
import {
  Observable,
  Subject,
  catchError,
  combineLatest,
  concat,
  concatMap,
  defer,
  finalize,
  forkJoin,
  from,
  map,
  mergeMap,
  of,
  reduce,
  switchMap,
  tap,
  toArray,
} from 'rxjs';
import { SemVer } from 'semver';

import { ToADC } from './transformer';
import * as typing from './typing';

function logTask<T>(
  taskName: string,
  task: Observable<T>,
): Observable<{ name: string; result: T }> {
  return defer(() => {
    console.log(`🚀 Start ${taskName}`);
    return task.pipe(
      tap({
        next: () => console.log(`🛠 Debug ${taskName}`),
        error: (err) => console.error(`❌ Error in ${taskName}:`, err),
        complete: () => console.log(`✅ End ${taskName}`),
      }),
      map((result) => ({ name: taskName, result })), // 任务完成后返回结果
    );
  });
}

export interface FetcherOptions {
  client: Axios;
  version: SemVer;
  eventEmitter: EventEmitter;
  backendOpts: ADCSDK.BackendOptions;
  gatewayGroupName?: string;
  gatewayGroupId?: string;
}
export class Fetcher {
  private readonly toADC = new ToADC();
  private readonly client: Axios;
  private readonly eventEmitter: EventEmitter;

  constructor(private readonly opts: FetcherOptions) {
    this.client = opts.client;
    this.eventEmitter = opts.eventEmitter;
  }

  public listServices(buffer: Array<Array<() => void>>) {
    if (this.isSkip(ADCSDK.ResourceType.SERVICE))
      return of<Array<ADCSDK.Service>>([]);

    const emits: Array<() => void> = [];
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
      tap((resp) =>
        emits.push(
          () =>
            this.emitTaskStateEvent(
              ADCSDK.BackendEventType.TASK_START,
              'Fetch services',
            ),
          () => this.emitDebugLog(resp, 'Get services'),
        ),
      ),
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
                emits.push(() =>
                  this.emitDebugLog(
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
      tap(() =>
        emits.push(() =>
          this.emitTaskStateEvent(
            ADCSDK.BackendEventType.TASK_DONE,
            'Fetch services',
          ),
        ),
      ),
      tap(() => buffer.push(emits)),
    );
  }

  public listConsumers(buffer: Array<Array<() => void>>) {
    if (this.isSkip(ADCSDK.ResourceType.CONSUMER))
      return of<Array<ADCSDK.Consumer>>([]);

    const emits: Array<() => void> = [];
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
      tap((resp) =>
        emits.push(
          () =>
            this.emitTaskStateEvent(
              ADCSDK.BackendEventType.TASK_START,
              'Fetch consumers',
            ),
          () => this.emitDebugLog(resp, 'Get consumers'),
        ),
      ),
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
                emits.push(() =>
                  this.emitDebugLog(
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
      tap(() =>
        emits.push(() =>
          this.emitTaskStateEvent(
            ADCSDK.BackendEventType.TASK_DONE,
            'Fetch consumers',
          ),
        ),
      ),
      tap(() => buffer.push(emits)),
    );
  }

  public listSSLs(buffer: Array<Array<() => void>>) {
    if (this.isSkip(ADCSDK.ResourceType.SSL)) return of<Array<ADCSDK.SSL>>([]);

    const emits: Array<() => void> = [];
    return from(
      this.client.get<{ list: Array<typing.SSL> }>('/apisix/admin/ssls', {
        params: this.attachLabelSelector({
          gateway_group_id: this.opts.gatewayGroupId,
        }),
      }),
    ).pipe(
      tap((resp) =>
        emits.push(
          () =>
            this.emitTaskStateEvent(
              ADCSDK.BackendEventType.TASK_START,
              'Fetch ssls',
            ),
          () => this.emitDebugLog(resp, 'Get ssls'),
        ),
      ),
      mergeMap((resp) =>
        from(resp.data.list).pipe(map((ssl) => this.toADC.transformSSL(ssl))),
      ),
      toArray(),
      tap(() =>
        emits.push(() =>
          this.emitTaskStateEvent(
            ADCSDK.BackendEventType.TASK_DONE,
            'Fetch ssls',
          ),
        ),
      ),
      tap(() => buffer.push(emits)),
    );
  }

  public listGlobalRules(buffer: Array<Array<() => void>>) {
    if (this.isSkip(ADCSDK.ResourceType.GLOBAL_RULE))
      return of<Record<string, ADCSDK.GlobalRule>>({});

    const emits: Array<() => void> = [];
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
      tap((resp) =>
        emits.push(
          () =>
            this.emitTaskStateEvent(
              ADCSDK.BackendEventType.TASK_START,
              'Fetch global rules',
            ),
          () => this.emitDebugLog(resp, 'Get global rules'),
        ),
      ),
      map((resp) => this.toADC.transformGlobalRule(resp?.data?.list ?? [])),
      tap(() =>
        emits.push(() =>
          this.emitTaskStateEvent(
            ADCSDK.BackendEventType.TASK_DONE,
            'Fetch global rules',
          ),
        ),
      ),
      tap(() => buffer.push(emits)),
    );
  }

  public listMetadatas(buffer: Array<Array<() => void>>) {
    if (this.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA))
      return of<Record<string, ADCSDK.PluginMetadata>>({});

    const emits: Array<() => void> = [];
    return from(
      this.client.get<{
        value: ADCSDK.Plugins;
      }>('/apisix/admin/plugin_metadata', {
        params: this.attachLabelSelector({
          gateway_group_id: this.opts.gatewayGroupId,
        }),
      }),
    ).pipe(
      tap((resp) =>
        emits.push(
          () =>
            this.emitTaskStateEvent(
              ADCSDK.BackendEventType.TASK_START,
              'Fetch plugin metadata',
            ),
          () => this.emitDebugLog(resp, 'Get plugin metadata'),
        ),
      ),
      map((resp) => this.toADC.transformPluginMetadatas(resp?.data?.value)),
      tap(() =>
        emits.push(() =>
          this.emitTaskStateEvent(
            ADCSDK.BackendEventType.TASK_DONE,
            'Fetch plugin metadata',
          ),
        ),
      ),
      tap(() => buffer.push(emits)),
    );
  }

  public allTask() {
    const emitterBuffer: Array<Array<() => void>> = [];
    return from([
      { task: this.listServices(emitterBuffer) },
      { task: this.listConsumers(emitterBuffer) },
      { task: this.listSSLs(emitterBuffer) },
      { task: this.listGlobalRules(emitterBuffer) },
      { task: this.listMetadatas(emitterBuffer) },
    ]).pipe(
      mergeMap(({ task }) => task),
      toArray(),
      tap(() =>
        emitterBuffer.forEach((emits) => emits.forEach((emit) => emit())),
      ),
      map(() => ({}) as ADCSDK.Configuration),
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

  private emitDebugLog(response: AxiosResponse, description?: string) {
    this.eventEmitter.emit(ADCSDK.BackendEventType.AXIOS_DEBUG, {
      response,
      description,
    } satisfies ADCSDK.BackendEventAxiosDebug);
  }

  private emitTaskStateEvent(
    type:
      | typeof ADCSDK.BackendEventType.TASK_START
      | typeof ADCSDK.BackendEventType.TASK_DONE,
    name: string,
  ) {
    this.eventEmitter.emit(type, {
      name,
    } satisfies ADCSDK.BackendEventTaskState);
  }
}
